#!/usr/bin/env python3
"""completion_gate.py -- manifest-driven 3-state completion gate (plan_26072506 Phase 1,
independent-review remediation pass -- third cycle, subagent-summary-1-20260725_085654_777234).

Consumes a work-manifest (.claude/schemas/work-manifest.schema.json) describing one unit of
work's identity, runtime state, evidence artifacts, benchmark verdict, and PII scan result;
emits a completion-manifest (.claude/schemas/completion-manifest.schema.json) on stdout as
stable (sort_keys) JSON, deciding whether the work is eligible for promotion (hint/last-good/
branch-sync side effects, gated in a later phase).

Design (post-review):
  - Schema-shape validation is a small GENERIC Draft-07-subset engine (validate_against_schema)
    driven entirely by the three .schema.json files loaded from disk at import time -- there is no
    second, hand-duplicated list of "which fields must look like what". It implements exactly the
    keywords those schema files use: $ref (local), type (incl. union types and correct
    bool-vs-int/number handling), required, properties, additionalProperties (both the `false`
    and the `{schema}` forms), enum, const, minimum, minLength, minItems, items, allOf, if/then/
    else (the last three drive completion-manifest.schema.json's `state == 'promotion-ready' iff
    eligible_for_promotion == true` and side-effect-authorization.schema.json's analogous
    `authorization_state`/`allowed`/`exit_code` cross-field invariants -- Phase 3, plan_26072506).
  - Business/operational logic (which evidence keys are required for which task_class, the
    promotion-capped set, the certificate field-name mapping) stays as small Python data tables
    -- these are the "operational matrices" the plan explicitly allows outside the schema; they
    are not a second copy of anything the schema itself declares.
  - Every evidence-style path (evidence.*.path and capacity_rejection.gate_evidence) is resolved
    through a hardened, descriptor-relative walk from an opened repo-root fd using O_NOFOLLOW at
    every path component (not just the leaf) -- a symlink anywhere in the chain is rejected by
    construction (open() itself fails with ELOOP), which removes the classic
    resolve-then-recheck TOCTOU window rather than trying to close it after the fact. The same
    open file descriptor used for the containment-safe stat is the one hashed/read. Every such
    path (plus capacity_rejection.gate_evidence and pii_scan.scanned_paths) is additionally
    rejected outright if it is an absolute string, checked BEFORE any join/normalization --
    manifests are location/machine-independent by contract, not merely by convention.
  - A required simlog-style directory must contain at least one non-empty regular file anywhere
    in its tree (checked recursively, in the same walk that computes its tree fingerprint) --
    an empty directory or an all-zero-byte-files tree is not "existing raw evidence". The tree
    fingerprint itself is a canonical, unambiguous byte encoding (type byte + fixed-size length
    prefix + raw name bytes + raw digest bytes, no separator) rather than delimiter-joined text,
    so no filename can be crafted to collide two different trees' fingerprints.
  - Certificate identity/verdict/benchmark_mode are trusted ONLY from parsing the actual flat-YAML
    certificate artifact on disk (parse_flat_certificate) -- the work-manifest schema does not
    even allow an `identity` sub-object on evidence.certificate any more (manifest-self-assertion
    cannot establish trust).

State machine (single source of truth -- no prose document self-judges state):
    null                pre-lifecycle sentinel: manifest is schema-invalid (exit 2) OR well-formed
                         but its runtime/capacity_rejection tier is not yet satisfied (exit 1).
                         NOT a 4th named lifecycle state -- see completion-manifest.schema.json's
                         top-level description for the full rationale.
    runtime-ready       healthy runtime tier (or valid capacity-rejection substitute), required
                         evidence/identity/PII checks for evidence-complete not yet satisfied
    evidence-complete   required evidence for this task_class present + identity (incl. the
                         parsed certificate artifact) verified + PII scan passed with full
                         coverage, but not eligible for promotion
    promotion-ready      evidence-complete AND full PASS bundle AND task_class not capped
                         (invariant: state == "promotion-ready" iff eligible_for_promotion == True)

Usage:
    python3 completion_gate.py verify --manifest <path> [--repo-root <path>]

Exit codes:
    0  success -- eligible_for_promotion == True
    1  policy-block -- manifest well-formed, evaluation completed, but not eligible for promotion
    2  invalid-input -- malformed JSON / missing or unreadable manifest file / repo-root not
       found / schema-shape violation / pre-state-machine structural contract violation (e.g.
       certificate present without a PASS verdict) / any evidence or capacity-gate path that
       escapes the repo or resolves through a symlink

No third-party dependencies (stdlib only) -- schema files are plain JSON, validated here with the
generic engine described above rather than the `jsonschema` package (no new-dependency installs
permitted this phase; see testlog design-rationale).
"""
from __future__ import annotations

import argparse
import datetime
import errno
import hashlib
import json
import math
import os
import re
import stat
import subprocess
import sys
import urllib.parse
from pathlib import Path

SCHEMA_VERSION = 1
_SCHEMA_DIR = Path(__file__).resolve().parents[2] / "schemas"


def _load_schema(filename: str) -> dict:
    with open(_SCHEMA_DIR / filename, "r", encoding="utf-8") as f:
        return json.load(f)


WORK_SCHEMA = _load_schema("work-manifest.schema.json")
COMPLETION_SCHEMA = _load_schema("completion-manifest.schema.json")
SIDE_EFFECT_SCHEMA = _load_schema("side-effect-authorization.schema.json")

# Phase 3 (plan_26072506): the single source of truth for which side-effect actions this gate
# knows about -- mirrored (not hand-duplicated business logic, just the same literal values) by
# work-manifest.schema.json's executionApproval.allowed_actions.items.enum and
# side-effect-authorization.schema.json's action enum; the owner-local production
# `runtime_selftest.py::_test_completion_gate` guards against drift between the three. The
# hint_* actions are the owner-local hint-publisher hint_tag.py subcommands that mutate git
# tags/hints/index.json/HINTS.md or push to a remote -- `match` stays deliberately absent (it is
# read-only and ungated by design, never wired to this gate).
ALLOWED_ACTIONS = (
    "sync_to_sub", "sync_branches",
    "hint_create", "hint_finalize", "hint_verify", "hint_reindex", "hint_push", "hint_reverify",
)


# =============================================================================
# Generic Draft-07-subset schema validator (A.1) -- driven entirely by the schema dict passed
# in, recursively, for arbitrary nesting depth. No per-field hand-rolled duplicate checks.
# =============================================================================

def _json_type_matches(value, json_type: str) -> bool:
    if json_type == "object":
        return isinstance(value, dict)
    if json_type == "array":
        return isinstance(value, list)
    if json_type == "string":
        return isinstance(value, str)
    if json_type == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if json_type == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if json_type == "boolean":
        return isinstance(value, bool)
    if json_type == "null":
        return value is None
    raise ValueError(f"completion_gate: schema uses unsupported type keyword {json_type!r}")


def _field(path: str, key) -> str:
    return f"{path}.{key}" if path else str(key)


def _resolve_ref(ref: str, root_schema: dict) -> dict:
    if not ref.startswith("#/"):
        raise ValueError(f"completion_gate: only local $ref is supported, got {ref!r}")
    node = root_schema
    for part in ref[2:].split("/"):
        node = node[part]
    return node


def validate_against_schema(instance, schema: dict, root_schema: dict | None = None, path: str = "") -> list[tuple[str, str]]:
    """Recursive Draft-07-subset validator. Implements exactly: $ref (local), type (union +
    bool-vs-int/number correctness), required, properties, additionalProperties (bool-false and
    schema forms), enum, const, minimum, minLength, items, allOf, if/then/else (OUTPUT_SCHEMA_
    INVARIANT: added so a schema can express a cross-field conditional invariant, e.g. `state ==
    'promotion-ready' iff eligible_for_promotion == true`, as two if/then blocks under allOf --
    the biconditional's two directions). Returns a list of (reason_code, message) violations,
    empty if `instance` conforms to `schema`.
    """
    if root_schema is None:
        root_schema = schema
    violations: list[tuple[str, str]] = []

    if "$ref" in schema:
        return validate_against_schema(instance, _resolve_ref(schema["$ref"], root_schema), root_schema, path)

    if "type" in schema:
        allowed = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(_json_type_matches(instance, t) for t in allowed):
            label = path or "$"
            violations.append((
                f"SCHEMA_TYPE_MISMATCH:{label}",
                f"{label}: expected type {allowed}, got {type(instance).__name__} ({instance!r})",
            ))
            return violations  # further keywords assume the type already matches

    if "allOf" in schema:
        for subschema in schema["allOf"]:
            violations.extend(validate_against_schema(instance, subschema, root_schema, path))

    if "if" in schema:
        if_violations = validate_against_schema(instance, schema["if"], root_schema, path)
        if not if_violations:
            if "then" in schema:
                violations.extend(validate_against_schema(instance, schema["then"], root_schema, path))
        elif "else" in schema:
            violations.extend(validate_against_schema(instance, schema["else"], root_schema, path))

    if "const" in schema:
        expected = schema["const"]
        if instance != expected or isinstance(instance, bool) != isinstance(expected, bool):
            label = path or "$"
            violations.append((f"SCHEMA_CONST_VIOLATION:{label}", f"{label}: must equal {expected!r}, got {instance!r}"))

    if "enum" in schema:
        allowed_values = schema["enum"]
        if not any(instance == v and isinstance(instance, bool) == isinstance(v, bool) for v in allowed_values):
            label = path or "$"
            violations.append((f"SCHEMA_ENUM_VIOLATION:{label}", f"{label}: {instance!r} is not one of {allowed_values!r}"))

    if isinstance(instance, str) and "minLength" in schema and len(instance) < schema["minLength"]:
        label = path or "$"
        violations.append((f"SCHEMA_MIN_LENGTH_VIOLATION:{label}",
                           f"{label}: length {len(instance)} < minLength {schema['minLength']}"))

    if isinstance(instance, (int, float)) and not isinstance(instance, bool) and "minimum" in schema and instance < schema["minimum"]:
        label = path or "$"
        violations.append((f"SCHEMA_MINIMUM_VIOLATION:{label}", f"{label}: {instance!r} < minimum {schema['minimum']}"))

    if isinstance(instance, list) and "minItems" in schema and len(instance) < schema["minItems"]:
        label = path or "$"
        violations.append((f"SCHEMA_MIN_ITEMS_VIOLATION:{label}",
                           f"{label}: length {len(instance)} < minItems {schema['minItems']}"))

    if isinstance(instance, dict):
        props = schema.get("properties", {})
        for key in schema.get("required", []):
            if key not in instance:
                violations.append((f"SCHEMA_MISSING_REQUIRED_KEY:{_field(path, key)}",
                                   f"{_field(path, key)}: required property is missing"))
        additional = schema.get("additionalProperties", True)
        if additional is False:
            for key in instance:
                if key not in props:
                    violations.append((f"SCHEMA_UNKNOWN_PROPERTY:{_field(path, key)}",
                                       f"{_field(path, key)}: unrecognized property (additionalProperties: false)"))
        for key, subschema in props.items():
            if key in instance:
                violations.extend(validate_against_schema(instance[key], subschema, root_schema, _field(path, key)))
        if isinstance(additional, dict):
            for key in instance:
                if key not in props:
                    violations.extend(validate_against_schema(instance[key], additional, root_schema, _field(path, key)))

    if isinstance(instance, list) and "items" in schema:
        for i, item in enumerate(instance):
            violations.extend(validate_against_schema(item, schema["items"], root_schema, f"{path}[{i}]"))

    return violations


# =============================================================================
# Hardened, symlink-rejecting, descriptor-relative evidence path resolution (C1/C2)
# =============================================================================

def _lexical_components(manifest_dir: Path, repo_root: Path, rel_path: str):
    """Pure string/path normalization, no filesystem I/O. Returns (status, parts):
      status="ok"           -> parts is the list of path components from repo_root to target
      status="repo_escape"  -> lexical '..' traversal leaves repo_root; parts is None
    """
    lexical = Path(os.path.normpath(str(manifest_dir / rel_path)))
    try:
        rel = lexical.relative_to(repo_root)
    except ValueError:
        return "repo_escape", None
    parts = [p for p in rel.parts if p not in ("", ".")]
    return "ok", parts


def _walk_nofollow(repo_root_fd: int, parts: list[str]):
    """Descriptor-relative walk from repo_root_fd through `parts`, O_NOFOLLOW at every
    component (not just the leaf) -- a symlink anywhere in the chain fails open() itself
    (ELOOP), so there is no separate resolve-then-recheck window to race. O_NONBLOCK guards
    every open() against hanging on a FIFO. Returns (status, fd_or_None); caller must
    os.close() the fd on status == "ok".
    """
    if not parts:
        return "not_found", None
    cur_fd = os.dup(repo_root_fd)
    for i, part in enumerate(parts):
        is_last = i == len(parts) - 1
        flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
        if not is_last:
            flags |= os.O_DIRECTORY
        try:
            next_fd = os.open(part, flags, dir_fd=cur_fd)
        except FileNotFoundError:
            os.close(cur_fd)
            return "not_found", None
        except NotADirectoryError:
            os.close(cur_fd)
            return "not_a_directory", None
        except OSError as e:
            os.close(cur_fd)
            if e.errno == errno.ELOOP:
                return "symlink", None
            return "os_error", None
        os.close(cur_fd)
        cur_fd = next_fd
    return "ok", cur_fd


def _hash_and_maybe_read(fd: int, capture_content: bool):
    """Hashes the given regular-file fd (streaming), optionally also capturing its full bytes,
    via a dup()'d descriptor so the caller's fd/position is untouched."""
    h = hashlib.sha256()
    f = os.fdopen(os.dup(fd), "rb")
    content = bytearray() if capture_content else None
    try:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
            if capture_content:
                content.extend(chunk)
    finally:
        f.close()
    return h.hexdigest(), (bytes(content) if capture_content else None)


def _tree_fingerprint(dir_fd: int):
    """Recursively fingerprints a directory already opened via a symlink-safe fd, rejecting any
    symlink/FIFO/device/socket anywhere in the tree. Returns (ok, hash_hex_or_None,
    reason_or_None, has_raw_evidence). No side effects -- read-only opens only.

    AMBIGUOUS_TREE_FINGERPRINT_ENCODING fix: each entry is framed as `type_byte(1) +
    name_length(4, big-endian) + raw_name_bytes + raw_32-byte_digest`, concatenated with NO
    separator -- the fixed-size length prefix makes every record self-delimiting, so no filename
    (however it is chosen, including one containing embedded newlines/spaces/other entries'
    hash text) can ever be reinterpreted as a different sequence of entries. The prior
    `"F {name} {hash}"` newline-joined text encoding was ambiguous by construction: a single
    crafted filename embedding a literal separator + another entry's hash text could reproduce
    a two-entry tree's exact combined string (a delimiter-injection collision, not a
    cryptographic hash break). `name` is re-encoded with the same surrogateescape handler
    os.listdir(str) used to decode it, recovering the exact original filename bytes.

    EMPTY_SIMLOG_PROMOTION_BYPASS fix: has_raw_evidence is True iff this tree contains at least
    one regular, non-empty file anywhere (recursively) -- an empty directory, or a tree
    containing only zero-byte files, has has_raw_evidence=False even though fingerprinting itself
    still succeeds."""
    try:
        names = sorted(os.listdir(dir_fd))
    except OSError:
        return False, None, "tree_unreadable", False
    records = bytearray()
    has_raw_evidence = False
    for name in names:
        try:
            st = os.lstat(name, dir_fd=dir_fd)
        except OSError:
            return False, None, "tree_unreadable", False
        if stat.S_ISLNK(st.st_mode):
            return False, None, "tree_contains_symlink", False
        name_bytes = name.encode("utf-8", "surrogateescape")
        name_len_prefix = len(name_bytes).to_bytes(4, "big")
        if stat.S_ISDIR(st.st_mode):
            try:
                sub_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=dir_fd)
            except OSError:
                return False, None, "tree_unreadable", False
            try:
                ok, sub_hash, reason, sub_has_raw = _tree_fingerprint(sub_fd)
            finally:
                os.close(sub_fd)
            if not ok:
                return False, None, reason, False
            has_raw_evidence = has_raw_evidence or sub_has_raw
            records += b"D" + name_len_prefix + name_bytes + bytes.fromhex(sub_hash)
        elif stat.S_ISREG(st.st_mode):
            try:
                fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=dir_fd)
            except OSError:
                return False, None, "tree_unreadable", False
            try:
                file_hash, _ = _hash_and_maybe_read(fd, capture_content=False)
            finally:
                os.close(fd)
            if st.st_size > 0:
                has_raw_evidence = True
            records += b"F" + name_len_prefix + name_bytes + bytes.fromhex(file_hash)
        else:
            return False, None, "tree_contains_special_file", False
    # `names` already sorted -> deterministic record ordering.
    return True, hashlib.sha256(bytes(records)).hexdigest(), None, has_raw_evidence


def resolve_and_stat_evidence(repo_root_fd: int, repo_root: Path, manifest_dir: Path, rel_path: str,
                               expect_dir: bool, capture_content: bool = False) -> dict:
    """Full hardened resolution for one evidence-style path (evidence.*.path or
    capacity_rejection.gate_evidence). No side effects (read-only opens; no writes/creates).
    Returns:
      status: "ok" | "repo_escape" | "symlink" | "not_found" | "not_a_directory" | "wrong_type" | "os_error"
      kind:   "file" | "dir" | None
      sha256: str | None        (regular files only)
      tree_sha256: str | None   (directories only)
      content_bytes: bytes | None  (regular files only, only when capture_content=True)
    """
    result = {"status": None, "kind": None, "sha256": None, "tree_sha256": None, "content_bytes": None}

    lex_status, parts = _lexical_components(manifest_dir, repo_root, rel_path)
    if lex_status == "repo_escape":
        result["status"] = "repo_escape"
        return result

    walk_status, fd = _walk_nofollow(repo_root_fd, parts)
    if walk_status != "ok":
        result["status"] = walk_status
        return result

    try:
        st = os.fstat(fd)
        if stat.S_ISDIR(st.st_mode):
            if not expect_dir:
                result["status"], result["kind"] = "wrong_type", "dir"
                return result
            ok, tree_hash, _reason, has_raw_evidence = _tree_fingerprint(fd)
            if not ok:
                result["status"], result["kind"] = "wrong_type", "dir"
                return result
            if not has_raw_evidence:
                # EMPTY_SIMLOG_PROMOTION_BYPASS fix: recognized as a real directory (tree_sha256
                # is still computed and surfaced for debuggability) but NOT "ok" -- an empty
                # directory, or a tree containing only zero-byte files, must never be treated as
                # existing raw evidence.
                result["status"], result["kind"], result["tree_sha256"] = "empty_tree", "dir", tree_hash
                return result
            result["status"], result["kind"], result["tree_sha256"] = "ok", "dir", tree_hash
            return result
        if stat.S_ISREG(st.st_mode):
            if expect_dir or st.st_size == 0:
                result["status"], result["kind"] = "wrong_type", "file"
                return result
            digest, content = _hash_and_maybe_read(fd, capture_content)
            result["status"], result["kind"], result["sha256"], result["content_bytes"] = "ok", "file", digest, content
            return result
        # FIFO / socket / char / block device -- never read from these
        result["status"] = "wrong_type"
        return result
    finally:
        os.close(fd)


# =============================================================================
# Required local-link validation for Markdown-like evidence (REQUIRED_LINK_VALIDATION_ABSENT /
# MARKDOWN_ANGLE_DESTINATION_LINK_BYPASS) -- deterministic, stdlib-only, fail-closed Markdown link
# scanning sufficient for CommonMark local destinations: ordinary inline `[text](target)`, angle-
# bracket destinations `[text](<target with spaces>)`, backslash-escaped characters, balanced
# nested parentheses in non-angle destinations, an optional inline title ("...", '...', or
# (...)), images `![alt](target)`, reference-style definitions (`[label]: target`) AND
# reference-style USES (`[text][label]`/`[text][]`). Fenced/inline code spans are blanked out of
# consideration first (never scanned). http(s)/mailto targets and same-document anchors are
# ignored (not local). Everything else is either a local path that must resolve safely (through
# the SAME hardened dir_fd/O_NOFOLLOW walk used for evidence paths -- no second, weaker path-
# safety mechanism) and exist, or is rejected outright (absolute path, malformed percent-encoding,
# malformed inline-link syntax, or a reference USE whose label has no matching definition) --
# unsupported/malformed link-like syntax fails closed, it is never silently skipped as if it were
# plain non-link text. Shortcut references (`[label]`/`![label]` with no following `(...)`/
# `[...]`) are treated as links ONLY when a matching normalized reference definition exists
# elsewhere in the document (MARKDOWN_SHORTCUT_REFERENCE_FALSE_NEGATIVE) -- ordinary bracketed
# prose with no matching definition is left untouched (matching every bracketed prose phrase as a
# potential link would be far too aggressive for arbitrary devlog/plan prose).
# =============================================================================

_EXTERNAL_SCHEME_RE = re.compile(r'^[a-zA-Z][a-zA-Z0-9+.\-]*://')
_VALID_PERCENT_ESCAPE_RE = re.compile(r'%[0-9A-Fa-f]{2}')
_ESCAPABLE_RE = re.compile(r'\\([!"#$%&\'()*+,\-./:;<=>?@\[\\\]^_`{|}~])')
_WINDOWS_ABS_RE = re.compile(r'^[A-Za-z]:[\\/]')
_FENCE_OPEN_RE = re.compile(r'^( {0,3})(`{3,}|~{3,})(.*)$')


def _is_absolute_path_string(path_str: str) -> bool:
    """POSIX-absolute (leading '/', which also covers the POSIX-style double-slash UNC form
    '//server/share'), Windows drive-absolute ('C:\\' / 'C:/'), Windows UNC ('\\\\server\\share'),
    and Windows root-relative ('\\rooted') -- checked on the raw string. Every form except
    drive-absolute begins with '/' or a bare backslash at index 0 (INCOMPLETE_ABSOLUTE_PATH_
    CONTRACT fix: a leading backslash was not checked at all before, so UNC and root-relative
    paths fell through as ordinary relative paths). Shared by classify_link_target (link
    destinations) and cmd_verify's evidence/gate/PII-scanned-path input-contract check
    (ABSOLUTE_EVIDENCE_PATHS_ACCEPTED) -- one definition, not a duplicated business rule."""
    return path_str.startswith("/") or path_str.startswith("\\") or bool(_WINDOWS_ABS_RE.match(path_str))


def _unescape_markdown(s: str) -> str:
    """CommonMark backslash-escapes: a backslash followed by one of the ASCII punctuation
    characters is that literal character; a backslash before anything else is left untouched.
    '%' is not in the escapable set, so percent-encoding survives this step unchanged."""
    return _ESCAPABLE_RE.sub(r'\1', s)


def _blank_inline_code_spans(text: str) -> str:
    """Replaces the content of every well-formed inline code span (a run of N backticks ... the
    next run of exactly N backticks) with spaces, preserving length/newlines so line-based
    reference-definition scanning is unaffected. A backtick run with no matching close is left
    as literal text (rare, not a code span)."""
    n = len(text)
    result = list(text)
    i = 0
    while i < n:
        if text[i] == '`':
            j = i
            while j < n and text[j] == '`':
                j += 1
            run_len = j - i
            k = j
            close_start = close_end = None
            while k < n:
                if text[k] == '`':
                    m = k
                    while m < n and text[m] == '`':
                        m += 1
                    if (m - k) == run_len:
                        close_start, close_end = k, m
                        break
                    k = m
                else:
                    k += 1
            if close_start is not None:
                for p in range(i, close_end):
                    if text[p] != '\n':
                        result[p] = ' '
                i = close_end
                continue
            i = j
            continue
        i += 1
    return ''.join(result)


def _closing_fence_re(fence_char: str, fence_len: int) -> re.Pattern:
    escaped = re.escape(fence_char)
    return re.compile(r'^ {0,3}' + escaped + '{' + str(fence_len) + r',}[ \t]*$')


def _blank_code_regions(text: str) -> str:
    """Blanks fenced code blocks to spaces line-by-line, then blanks inline code spans -- link
    syntax inside either is never scanned. MARKDOWN_FENCE_COMMONMARK_DIVERGENCE fix -- matches
    CommonMark exactly (verified against the markdown_it commonmark-preset reference renderer as
    an oracle, never imported here):
      opening: 0-3 leading spaces + a run of >=3 identical backticks/tildes. For a BACKTICK fence
        only, the rest of the line (the info string) must not itself contain a backtick -- if it
        does, this is not a valid fence opener at all (the line is ordinary text, scanned
        normally; a real broken link elsewhere must not be hidden by a rejected fence attempt).
      closing: 0-3 leading spaces + a run of the SAME character >= the opener's length, with
        nothing but spaces/tabs trailing -- a line that merely starts with a qualifying run but
        has other trailing content (e.g. ```not-a-close) does NOT close the fence; it is ordinary
        fence content and stays blanked."""
    lines = text.split('\n')
    out_lines = []
    fence_char = None
    fence_len = 0
    close_re = None
    for line in lines:
        if fence_char is not None:
            out_lines.append(' ' * len(line))
            if close_re.match(line):
                fence_char, fence_len, close_re = None, 0, None
            continue
        m = _FENCE_OPEN_RE.match(line)
        if m and not (m.group(2)[0] == '`' and '`' in m.group(3)):
            fence_char, fence_len = m.group(2)[0], len(m.group(2))
            close_re = _closing_fence_re(fence_char, fence_len)
            out_lines.append(' ' * len(line))
            continue
        out_lines.append(line)
    return _blank_inline_code_spans('\n'.join(out_lines))


def _scan_bracket(text: str, open_idx: int):
    """text[open_idx] == '['. Returns the index just after the matching ']' (nested-bracket- and
    backslash-escape-aware), or None if unterminated."""
    n = len(text)
    depth = 1
    j = open_idx + 1
    while j < n:
        c = text[j]
        if c == '\\' and j + 1 < n:
            j += 2
            continue
        if c == '[':
            depth += 1
        elif c == ']':
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return None


def _parse_destination(text: str, i: int):
    """Parses a link destination starting at index i. Returns (raw_destination, end_index, ok).
    Angle-bracket form `<...>` (may contain spaces; backslash-escapes honored; an unescaped '<'
    or a newline inside is malformed) is returned INCLUDING its `<`/`>` wrapper. Non-angle
    ("normal") form scans up to the first unescaped whitespace or unescaped ')' at paren-depth 0
    -- an unescaped '(' increases depth (balanced nested parentheses become part of the
    destination) and always succeeds (an empty destination, e.g. `[x]()`, is valid -- classify_
    link_target treats an empty target as skip/same-document, matching CommonMark)."""
    n = len(text)
    if i < n and text[i] == '<':
        j = i + 1
        while j < n:
            c = text[j]
            if c == '\\' and j + 1 < n:
                j += 2
                continue
            if c == '>':
                return text[i:j + 1], j + 1, True
            if c == '<' or c == '\n':
                return None, i, False
            j += 1
        return None, i, False
    j = i
    depth = 0
    while j < n:
        c = text[j]
        if c == '\\' and j + 1 < n:
            j += 2
            continue
        if c == '(':
            depth += 1
            j += 1
            continue
        if c == ')':
            if depth == 0:
                break
            depth -= 1
            j += 1
            continue
        if c.isspace():
            break
        j += 1
    return text[i:j], j, True


def _parse_title(text: str, i: int):
    """Parses an optional inline title ("...", '...', or (...)) starting at index i. Returns
    (raw_title_or_None, end_index) -- end_index == i (title None) if there is no title or it is
    unterminated (caller treats that as 'no title present', not a hard parse failure)."""
    n = len(text)
    if i >= n or text[i] not in ('"', "'", '('):
        return None, i
    open_c = text[i]
    close_c = ')' if open_c == '(' else open_c
    j = i + 1
    while j < n:
        c = text[j]
        if c == '\\' and j + 1 < n:
            j += 2
            continue
        if c == close_c:
            return text[i:j + 1], j + 1
        j += 1
    return None, i


def _skip_ws(text: str, i: int) -> int:
    """CommonMark inline-link 'spnl' whitespace: zero or more spaces/tabs, then OPTIONALLY one
    single line ending followed by zero or more further spaces/tabs. MARKDOWN_MULTIBLANK_INLINE_
    FALSE_POSITIVE fix: the prior unbounded `while text[i] in ' \\t\\n'` treated an entire blank
    line (two or more consecutive newlines) as ordinary skippable whitespace, letting inline-link
    destination/title parsing silently cross a paragraph boundary CommonMark never allows -- a
    blank line always terminates the current block before any inline parsing is attempted (matches
    commonmark.js/markdown_it's reference `spnl` rule and was verified against the markdown_it
    commonmark-preset renderer as an oracle: `[x](\\n\\ndefinitely-missing.md\\n)` renders as two
    literal-text paragraphs, never a link). Called at every whitespace gap in an inline link's
    `(...)` tail (before the destination, between destination/title, between title/close-paren) --
    each individual call still only ever consumes at most one line ending, so two consecutive
    newlines can never be crossed as a unit."""
    n = len(text)
    j = i
    while j < n and text[j] in ' \t':
        j += 1
    if j < n and text[j] == '\n':
        j += 1
        while j < n and text[j] in ' \t':
            j += 1
    return j


def _parse_inline_link_tail(text: str, i: int):
    """text[i-1] == '(' (i.e. i is the first char after the opening paren of an inline
    `[text](...)`/`![alt](...)` link). Returns (raw_destination, end_index, ok) where end_index
    is just past the link's closing ')'. ok=False means malformed (no valid destination, or no
    closing ')' after an optional title)."""
    j = _skip_ws(text, i)
    dest_raw, j, ok = _parse_destination(text, j)
    if not ok:
        return None, i, False
    k = _skip_ws(text, j)
    if k < len(text) and text[k] == ')':
        return dest_raw, k + 1, True
    _title_raw, k2 = _parse_title(text, k)
    if k2 == k:
        return None, i, False  # neither an immediate ')' nor a parseable title -- malformed
    k = _skip_ws(text, k2)
    if k < len(text) and text[k] == ')':
        return dest_raw, k + 1, True
    return None, i, False


def _normalize_label(label: str) -> str:
    return re.sub(r'\s+', ' ', label.strip()).casefold()


def _parse_reference_definitions(text: str) -> dict:
    """Returns {normalized_label: raw_destination}. One definition per matching line (0-3
    leading spaces, `[label]:` at line start, single-line destination + optional title). First
    definition for a given (normalized) label wins, matching CommonMark."""
    defs: dict[str, str] = {}
    for line in text.split('\n'):
        idx = 0
        while idx < len(line) and line[idx] == ' ':
            idx += 1
        if idx > 3 or idx >= len(line) or line[idx] != '[':
            continue
        label_end = _scan_bracket(line, idx)
        if label_end is None or label_end >= len(line) or line[label_end] != ':':
            continue
        label_raw = line[idx + 1:label_end - 1]
        j = _skip_ws(line, label_end + 1)
        dest_raw, _j2, ok = _parse_destination(line, j)
        if not ok:
            continue
        label_norm = _normalize_label(label_raw)
        if label_norm and label_norm not in defs:
            defs[label_norm] = dest_raw
    return defs


def _scan_markdown_links(text: str):
    """Deterministically scans one document's text for Markdown link-like constructs (after
    blanking fenced/inline code). Returns (raw_targets, unresolved_refs):
      raw_targets    -- destination strings (exactly as written, angle-bracket wrapper included
                         when present) from inline links/images and resolved reference uses,
                         ready for classify_link_target.
      unresolved_refs -- raw `[text][label]`/`[text][]` occurrences whose label has no matching
                         reference definition, and malformed inline destinations -- both fail
                         closed (the caller treats every entry here as invalid/broken), never
                         silently skipped.
    """
    # CommonMark treats LF, CRLF, and bare CR as equivalent line endings. Canonicalize once at
    # the scanner boundary so fence closing and inline-link spnl rules cannot diverge by platform.
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    sanitized = _blank_code_regions(text)
    ref_defs = _parse_reference_definitions(sanitized)
    raw_targets: list[str] = []
    unresolved: list[str] = []
    n = len(sanitized)
    i = 0
    while i < n:
        c = sanitized[i]
        if c == '\\' and i + 1 < n:
            i += 2
            continue
        is_image = False
        bracket_idx = None
        if c == '[':
            bracket_idx = i
        elif c == '!' and i + 1 < n and sanitized[i + 1] == '[':
            is_image, bracket_idx = True, i + 1
        if bracket_idx is None:
            i += 1
            continue
        label_end = _scan_bracket(sanitized, bracket_idx)
        if label_end is None:
            i = bracket_idx + 1
            continue
        label_text = sanitized[bracket_idx + 1:label_end - 1]
        if label_end < n and sanitized[label_end] == '(':
            dest_raw, end_idx, ok = _parse_inline_link_tail(sanitized, label_end + 1)
            if ok:
                raw_targets.append(dest_raw)
                i = end_idx
            else:
                unresolved.append(f"{'!' if is_image else ''}[{label_text}](...) [malformed inline destination]")
                i = label_end + 1
            continue
        if label_end < n and sanitized[label_end] == '[':
            label2_end = _scan_bracket(sanitized, label_end)
            if label2_end is not None:
                label2_text = sanitized[label_end + 1:label2_end - 1]
                ref_label = label2_text if label2_text.strip() else label_text
                norm = _normalize_label(ref_label)
                if norm in ref_defs:
                    raw_targets.append(ref_defs[norm])
                else:
                    unresolved.append(f"[{label_text}][{label2_text}]")
                i = label2_end
                continue
        # bare shortcut `[label]`/`![label]` (no following '(' or '[') -- MARKDOWN_SHORTCUT_
        # REFERENCE_FALSE_NEGATIVE fix: recognized and validated ONLY when a matching normalized
        # reference definition exists (verified against the markdown_it commonmark-preset
        # renderer as an oracle: `[missing]` + `[missing]: target` DOES render as a real link/
        # image). Ordinary bracketed prose with no matching definition is left untouched --
        # still out of scope, matching every bracketed-phrase-in-prose case that isn't reference
        # syntax at all.
        norm = _normalize_label(label_text)
        if norm in ref_defs:
            raw_targets.append(ref_defs[norm])
        i = label_end
    return raw_targets, unresolved


def classify_link_target(raw_target: str):
    """Returns (kind, payload):
      ("skip", None)       -- external URI (any scheme://), mailto:, same-document anchor, or
                               empty -- not a local reference, never validated
      ("invalid", reason)  -- malformed before any filesystem interaction: bad percent-encoding or
                               an absolute filesystem path
      ("local", path)      -- a decoded, repo-relative path candidate to resolve and require exists
    """
    target = raw_target.strip()
    if len(target) >= 2 and target[0] == "<" and target[-1] == ">":
        target = target[1:-1]
    target = _unescape_markdown(target)
    if not target or target.startswith("#") or target.lower().startswith("mailto:") or _EXTERNAL_SCHEME_RE.match(target):
        return "skip", None
    path_part = target.split("#", 1)[0].split("?", 1)[0]
    if not path_part:
        return "skip", None  # was purely an anchor/query with nothing local to resolve
    # urllib.parse.unquote is lenient about syntactically malformed escapes (e.g. "%zz" or a bare
    # "%" are left untouched rather than raising) -- it only raises when a SYNTACTICALLY valid
    # %XX triplet decodes to a byte sequence that isn't valid UTF-8. Catch the syntax case
    # ourselves first: strip every well-formed %XX escape: any '%' left over is malformed.
    if "%" in _VALID_PERCENT_ESCAPE_RE.sub("", path_part):
        return "invalid", "malformed percent-encoding"
    try:
        decoded = urllib.parse.unquote(path_part, errors="strict")
    except (UnicodeDecodeError, ValueError):
        return "invalid", "malformed percent-encoding"
    if _is_absolute_path_string(decoded):
        return "invalid", "absolute filesystem path"
    return "local", decoded


def _resolve_link_target(repo_root_fd: int, repo_root: Path, base_dir: Path, rel_path: str) -> str:
    """Existence-only hardened resolution for a link target (relative to the evidence file's OWN
    directory, per Markdown convention -- not the manifest's directory). Reuses the same lexical-
    escape-check + O_NOFOLLOW descriptor-relative walk as evidence paths; no hashing (not needed
    for a link, just safe existence). Returns a status string; "ok" is the only success."""
    lex_status, parts = _lexical_components(base_dir, repo_root, rel_path)
    if lex_status == "repo_escape":
        return "repo_escape"
    walk_status, fd = _walk_nofollow(repo_root_fd, parts)
    if walk_status != "ok":
        return walk_status
    try:
        st = os.fstat(fd)
    finally:
        os.close(fd)
    if stat.S_ISREG(st.st_mode) or stat.S_ISDIR(st.st_mode):
        return "ok"
    return "wrong_type"


def check_required_local_links(repo_root_fd: int, repo_root: Path, base_dir: Path, content_bytes: bytes):
    """Scans one evidence file's already-hashed content for local link targets, using the
    deterministic Markdown link scanner (_scan_markdown_links). Returns (unreadable, broken,
    invalid): unreadable=True means the content was not valid UTF-8 (fail closed, links could not
    even be extracted); broken/invalid are lists of raw target strings. 'invalid' also carries
    unresolved reference-style uses and malformed inline-link syntax (MARKDOWN_ANGLE_DESTINATION_
    LINK_BYPASS: unsupported/malformed link-like syntax fails closed, never silently skipped)."""
    try:
        text = content_bytes.decode("utf-8")
    except UnicodeDecodeError:
        return True, [], []
    broken: list[str] = []
    raw_targets, unresolved_refs = _scan_markdown_links(text)
    invalid: list[str] = list(unresolved_refs)
    for raw_target in raw_targets:
        kind, payload = classify_link_target(raw_target)
        if kind == "skip":
            continue
        if kind == "invalid":
            invalid.append(raw_target)
            continue
        status = _resolve_link_target(repo_root_fd, repo_root, base_dir, payload)
        if status != "ok":
            broken.append(raw_target)
    return False, broken, invalid


# =============================================================================
# Flat-scalar certificate parser (B3) -- stdlib-only, fails closed on anything not exactly the
# publish_benchmark_record.py flat contract (comments, blank lines, one unindented `key: value`
# per line; no nesting/lists/anchors).
# =============================================================================

_FLAT_LINE_RE = re.compile(r"^([A-Za-z0-9_]+):\s*(.*)$")

# manifest identity field -> certificate artifact field name (docs.md 'flat 계약' strong keys)
CERTIFICATE_FIELD_MAP = {
    "model": "model",
    "gpu": "gpu_model",
    "vllm": "vllm_version",
    "quant": "quantization",
    "topology": "topology",
    "tp": "tensor_parallel_size",
}

STRONG_IDENTITY_FIELDS = ("model", "gpu", "vllm", "quant", "topology", "tp")

# ---- certificate run key (plan_26090410 §3.2) -----------------------------------------------
# 인증서 한 장을 **"측정 하나"** 로 식별하는 키 = 강한 6키 + `measured_utc`. 소프트 지문(image_digest
# 등)은 넣지 않는다 — 넣는 순간 두 번째 identity 가 된다. 이 키가 곧 폴더 해소의 질의이자 tripwire ④
# 의 그룹 키다: **같은 술어를 감지기와 해소기가 공유**한다(갈라지면 둘이 다른 답을 낸다).
# 2026-09-04 실측: 추적 인증서 53장에서 이 키의 그룹핑이 바이트 동일 8쌍과 정확히 일치했다.
CERTIFICATE_RUN_KEY_FIELDS = tuple(CERTIFICATE_FIELD_MAP[f] for f in STRONG_IDENTITY_FIELDS) + ("measured_utc",)


def certificate_run_key(cert_fields: dict):
    """(6 strong cert fields + measured_utc) as a tuple, or None when any is absent/blank -- callers
    must treat None as *unidentifiable* (fail-closed), never as "matches nothing"."""
    vals = []
    for k in CERTIFICATE_RUN_KEY_FIELDS:
        v = cert_fields.get(k)
        if v is None or str(v).strip() == "":
            return None
        vals.append(str(v).strip())
    return tuple(vals)

# ---- benchmark-certificate rubric contract (plan_26082219 D5) --------------------------------
# 벤치 판정기(adversarial-benchmark/scripts/verdict_rule.py)가 `--target-tps 0` 같은 상수 0 을
# 정본으로 낙찰시키면 floor=0 이 되어 **어떤 측정치도 통과하는 PASS**(공허 PASS)가 만들어졌고, 그
# PASS 가 여기서 promotion-ready(=hint 발행 자격)를 열었다(2026-08-22 실측). 발생 원천은 판정기에서
# 막혔지만(D1), 승격 경로에는 두 잔여 위험이 남는다 -- ① 손저작/개조된 인증서, ② **수정 전에 발행된
# stale 인증서**. 그래서 판정기의 자기주장이 아니라 **디스크의 인증서 아티팩트 수치**로 여기서 한 번 더
# 계약을 건다: floor 는 유한 양수여야 하고, ratio 와 primary_source 는 실재해야 한다.
# floor>0 없이는 공허 PASS 가 수치적으로 성립하지 않으므로, primary_source 문자열 매칭(하드코딩 결함
# 칸)에 기대지 않고 이 두 수치만으로 배제된다 -- source 는 표면화만 한다.
RUBRIC_AUTHORITIES = ("weak", "explicit", "explore")

# ---- manifest-carried rubric contract (plan_26082405) ----------------------------------------
# 인증서는 **PASS 때만** 발행된다(publish_benchmark_record.py — 그 규칙은 유지한다). 그래서 REFUTE
# 런에는 인증서가 없고, 인증서에서만 읽던 rubric_authority 는 REFUTE 에서 영원히 None 이었다 —
# 아래 explore 자동개방 경로(U1)가 **죽은 코드**였다(2026-08-24 실측: bench report 는 판정기 산출대로
# "루브릭 권한 = explore" 를 적는데 게이트 입력으로 전파되지 않음 = 출처는 있는데 통로가 끊긴 침묵 누락).
# 처방은 조건 완화가 아니라 **carrier 교체**다: REFUTE 에서도 살아있는 채널(manifest.benchmark)이
# 같은 사실을 나른다. 계약은 인증서와 **글자 그대로 동일**하다 —
#   floor 유한 양수(공허 PASS 배제 불변) ∧ ratio 유한 ∧ primary_source 실재 ∧ authority 값역.
# 여기에 하나가 더 붙는다: **출처 표시**(`rubric_source`). 인증서는 그 자체가 결정론 발행기의
# 산출물이라 출처가 아티팩트로 증명되지만, manifest 는 사람도 쓸 수 있는 평면이다. 출처 표시가
# 없으면 손저작 값과 판정기 파생분이 데이터에서 구분되지 않아 `합성 금지`가 집행 불가가 된다.
MANIFEST_RUBRIC_SOURCES = ("verdict_json",)


def _manifest_rubric_contract(benchmark) -> dict:
    """manifest.benchmark 가 나른 rubric 사실을 인증서와 동일한 계약으로 판정한다(fail-closed).

    반환 dict:
      declared -- 이 채널을 **쓰겠다고 선언**했는가(`rubric_authority` 가 실재하는가).
                  False 는 결함이 아니라 '미사용'이다(부재와 결측의 구분) — 호출부는 조용히
                  fallback 을 포기하고 종전 경로(perf_waiver)로 간다. legacy manifest 가 스키마
                  성장만으로 붉어지지 않는다.
      ok       -- 선언분이 계약 전부를 통과했는가. explore 자동개방의 전제.
      problems -- (reason_code, message) 목록. 선언해 놓고 못 지킨 것은 **표면화**한다
                  (침묵 무시 ✗). 단 evidence-complete 는 막지 않는다 — 이 채널은 승격의
                  *추가 통로*이지 증거 아티팩트가 아니며, 막으면 종전 REFUTE 런의 상태가
                  뒤로 후퇴한다.
    """
    out = {"declared": False, "ok": False, "authority": None, "floor_tps": None,
           "ratio_M_over_primary": None, "primary_source": None, "problems": []}
    if not isinstance(benchmark, dict):
        return out
    raw_authority = benchmark.get("rubric_authority")
    raw_authority = raw_authority.strip() if isinstance(raw_authority, str) else ""
    if not raw_authority or raw_authority.upper() == "N/A":
        return out  # 채널 미사용 — 조용히 종전 경로로
    out["declared"] = True

    problems = out["problems"]
    if raw_authority in RUBRIC_AUTHORITIES:
        out["authority"] = raw_authority
    else:
        problems.append((
            "MANIFEST_RUBRIC_AUTHORITY_UNKNOWN",
            f"benchmark.rubric_authority={raw_authority!r} is not one of {list(RUBRIC_AUTHORITIES)} "
            f"-- an unknown rubric authority cannot be reasoned about, fail closed"))

    raw_source = benchmark.get("rubric_source")
    raw_source = raw_source.strip() if isinstance(raw_source, str) else ""
    if raw_source not in MANIFEST_RUBRIC_SOURCES:
        problems.append((
            "MANIFEST_RUBRIC_PROVENANCE_MISSING",
            f"benchmark.rubric_source={benchmark.get('rubric_source')!r} is not one of "
            f"{list(MANIFEST_RUBRIC_SOURCES)} -- 출처 표시가 없는 rubric 값은 판정기 산출물 파생분과 "
            f"손저작을 데이터에서 구분할 수 없다(헌법 §결정론 규율). 승격을 열지 않는다"))

    floor_tps = _certificate_number(benchmark.get("floor_tps"))
    out["floor_tps"] = floor_tps
    if floor_tps is None or floor_tps <= 0:
        problems.append((
            "MANIFEST_RUBRIC_FLOOR_INVALID",
            f"benchmark.floor_tps={benchmark.get('floor_tps')!r} is not a finite positive number "
            f"-- floor<=0 means the rubric imposed NO threshold (공허 PASS): every measurement "
            f"passes. Not promotable in ANY authority."))

    ratio_value = _certificate_number(benchmark.get("ratio_M_over_primary"))
    out["ratio_M_over_primary"] = ratio_value
    if ratio_value is None:
        problems.append((
            "MANIFEST_RUBRIC_RATIO_MISSING",
            f"benchmark.ratio_M_over_primary={benchmark.get('ratio_M_over_primary')!r} is not a "
            f"finite number -- the measurement-vs-rubric indicator is absent, so the verdict "
            f"cannot be audited"))

    primary_source = benchmark.get("primary_source")
    primary_source = primary_source.strip() if isinstance(primary_source, str) else ""
    out["primary_source"] = primary_source or None
    if not primary_source or primary_source.upper() == "N/A":
        problems.append((
            "MANIFEST_RUBRIC_SOURCE_MISSING",
            f"benchmark.primary_source={benchmark.get('primary_source')!r} is absent -- which "
            f"rubric slot won cannot be established (surfaced, never matched as a string: the "
            f"numeric floor/ratio contract above is the gate)"))

    out["ok"] = not problems
    return out


def _certificate_number(raw):
    """Parses ONE flat-certificate scalar into a finite float. Returns None for absent / empty /
    'N/A' / non-numeric / NaN / +-Inf -- i.e. every shape that cannot serve as a threshold. Pure
    parser: the caller decides what a None means (fail-closed at each call site), which is the
    '순수 파서의 None' 정당 칸 rather than a silent fallback."""
    if raw is None:
        return None
    text = str(raw).strip()
    if not text or text.upper() == "N/A":
        return None
    try:
        value = float(text)
    except ValueError:
        return None
    if math.isnan(value) or math.isinf(value):
        return None
    return value


def parse_flat_certificate(text: str):
    """Returns (fields: dict[str,str], ok: bool). ok=False (fields=={}) means the artifact's
    format was not recognized -- fail closed, never guess."""
    fields: dict[str, str] = {}
    for raw_line in text.splitlines():
        line = raw_line.rstrip("\r\n")
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if line != stripped:
            return {}, False  # any indentation => nesting => unsupported by this flat parser
        m = _FLAT_LINE_RE.match(line)
        if not m:
            return {}, False
        key, value = m.group(1), m.group(2).strip()
        if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
            value = value[1:-1].replace('\\"', '"')
        if key in fields:
            return {}, False  # duplicate key -- ambiguous, fail closed
        fields[key] = value
    return fields, True


def _sibling_certificates_with_key(repo_root_fd: int, repo_root: Path, manifest_dir: Path,
                                   cert_rel_path: str, run_key: tuple) -> list[dict]:
    """인증서가 놓인 디렉터리의 다른 `benchmark_*.yaml` 중 **같은 측정 키**를 가진 것들.
    plan_26090410 §3.2 — 2건+ 는 같은 측정이 두 파일로 추적됐다는 뜻이고(중복층), 이 게이트가
    tripwire ④ 뒤의 두 번째 방어선이다. 반환: [{path(repo-relative), sha256}] · 자기 자신 제외.
    파싱 불가·키 결측 형제는 매치 대상이 아니지만 삼키지 않고 stderr 에 남긴다(부재≠결측)."""
    lex_status, parts = _lexical_components(manifest_dir, repo_root, cert_rel_path)
    if lex_status != "ok" or not parts:
        return []
    dir_parts, self_name = parts[:-1], parts[-1]
    dir_path = repo_root.joinpath(*dir_parts) if dir_parts else repo_root
    try:
        dir_fd = os.open(str(dir_path), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    except OSError:
        return []
    try:
        names = sorted(os.listdir(dir_fd))
    except OSError:
        names = []
    finally:
        os.close(dir_fd)
    out: list[dict] = []
    for name in names:
        if name == self_name or not (name.startswith("benchmark_") and name.endswith(".yaml")):
            continue
        rel = "/".join([*dir_parts, name])
        r = resolve_and_stat_evidence(repo_root_fd, repo_root, repo_root, rel, expect_dir=False, capture_content=True)
        if r["status"] != "ok":
            print(f"[completion_gate] WARN sibling certificate {rel} unreadable ({r['status']}) -- excluded "
                  f"from run-key resolution (not silently treated as non-matching)", file=sys.stderr)
            continue
        try:
            fields, ok = parse_flat_certificate((r["content_bytes"] or b"").decode("utf-8"))
        except UnicodeDecodeError:
            ok, fields = False, {}
        if ok and certificate_run_key(fields) == run_key:
            out.append({"path": rel, "sha256": r["sha256"]})
    return out


def _certificate_strong_identity_matches(cert_fields: dict, manifest_identity: dict):
    """Compares the 6 strong identity fields between a parsed certificate and the manifest's own
    identity block. Returns (mapped_identity: dict[str,str|None], mismatched_fields: list[str])."""
    mapped: dict[str, str | None] = {}
    mismatched: list[str] = []
    for f in STRONG_IDENTITY_FIELDS:
        cert_key = CERTIFICATE_FIELD_MAP[f]
        cert_val = cert_fields.get(cert_key)
        mapped[f] = cert_val
        if f == "tp":
            try:
                cert_tp = int(cert_val)
            except (TypeError, ValueError):
                mismatched.append(f)
                continue
            if cert_tp != manifest_identity.get("tp"):
                mismatched.append(f)
        else:
            manifest_val = manifest_identity.get(f)
            manifest_cmp = "N/A" if manifest_val is None else str(manifest_val)
            if cert_val != manifest_cmp:
                mismatched.append(f)
    return mapped, mismatched


# =============================================================================
# Operational matrices (task_class -> required evidence; not schema constraints)
# =============================================================================

BASE_REQUIRED_EVIDENCE: dict[str, tuple[str, ...]] = {
    "model_serving_strategy": ("plan", "devlog", "simlog", "testlog"),
    "full_benchmark": ("plan", "devlog", "simlog", "testlog", "bench_report"),
    "harness_change": ("plan", "devlog", "testlog"),
    "capacity_rejection": ("plan",),  # + OR-group(devlog,testlog), special-cased below
    "minor_patch": ("verification", "commit"),
    "read_only_audit": (),
}

PROMOTION_CAPPED_TASK_CLASSES = {"capacity_rejection", "minor_patch", "read_only_audit"}

DIR_EVIDENCE_KEYS = {"simlog"}  # the only evidence kind that must be a real directory

# Evidence kinds whose content is scanned for local links when required (REQUIRED_LINK_VALIDATION_ABSENT).
# certificate is deliberately excluded -- it is a different, flat-scalar format, not Markdown.
MARKDOWN_LIKE_EVIDENCE_KEYS = {"plan", "devlog", "testlog", "bench_report", "report", "verification", "commit"}


def required_evidence_for(task_class: str, conditions: dict, verdict) -> list[str]:
    required = list(BASE_REQUIRED_EVIDENCE.get(task_class, ()))
    conditions = conditions or {}
    if task_class == "harness_change" and conditions.get("actual_trial") is True:
        required.append("simlog")
    if task_class == "read_only_audit" and conditions.get("report_requested") is True:
        required.append("report")
    if task_class == "full_benchmark" and verdict == "PASS":
        required.append("certificate")
    return required


# =============================================================================
# CLI plumbing / stable-JSON-on-every-expected-error contract
# =============================================================================

def _bare_result(code: str, message: str) -> dict:
    return {
        "schema_version": SCHEMA_VERSION, "task_class": None, "state": None,
        "eligible_for_promotion": False, "reason_codes": [code], "messages": {code: message},
        "checked_evidence": {}, "identity": None, "certificate": None,
    }


def _emit_with_schema(obj: dict, exit_code: int, schema: dict) -> None:
    """Shared stable-JSON-on-stdout / real-process-exit primitive behind both `verify`
    (COMPLETION_SCHEMA, via _emit below) and `authorize` (SIDE_EFFECT_SCHEMA, via
    _emit_authorization) -- one place that prints+validates+exits, two schema-specific thin
    wrappers so each subcommand keeps self-validating its OWN output contract."""
    obj["exit_code"] = exit_code
    violations = validate_against_schema(obj, schema)
    if violations:
        raise RuntimeError(
            f"completion_gate: internal bug -- own output violates {schema.get('title', '?')}.schema.json: "
            + "; ".join(f"{c}: {m}" for c, m in violations)
        )
    print(json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2))
    raise SystemExit(exit_code)


def _emit(obj: dict, exit_code: int) -> None:
    obj.setdefault("certificate", None)
    _emit_with_schema(obj, exit_code, COMPLETION_SCHEMA)


class _ManifestLoadError(Exception):
    """Carries a stable (reason_code, message) pair for a manifest read/parse failure, so the two
    callers (_load_manifest for `verify`'s COMPLETION_SCHEMA-shaped errors, and `authorize`'s
    SIDE_EFFECT_SCHEMA-shaped errors) can each wrap it into their own output shape instead of one
    read/parse path being hardwired to a single output schema."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


def _read_manifest_text(path: Path) -> str:
    try:
        with open(path, "r", encoding="utf-8") as f:
            return f.read()
    except FileNotFoundError:
        raise _ManifestLoadError("MANIFEST_FILE_NOT_FOUND", f"manifest file not found: {path}")
    except IsADirectoryError:
        raise _ManifestLoadError("MANIFEST_IS_A_DIRECTORY", f"manifest path is a directory, not a file: {path}")
    except UnicodeDecodeError as e:
        raise _ManifestLoadError("MANIFEST_INVALID_UTF8", f"manifest file is not valid UTF-8: {e}")
    except OSError as e:
        raise _ManifestLoadError("MANIFEST_IO_ERROR", f"could not read manifest file {path}: {e}")


def _parse_manifest_json(text: str):
    try:
        return json.loads(text)
    except json.JSONDecodeError as e:
        raise _ManifestLoadError("MANIFEST_INVALID_JSON", f"manifest is not valid JSON: {e}")


def _load_manifest(path: Path):
    try:
        text = _read_manifest_text(path)
        return _parse_manifest_json(text)
    except _ManifestLoadError as e:
        _emit(_bare_result(e.code, e.message), 2)


def _find_repo_root(start: Path) -> Path | None:
    cur = start.resolve()
    for candidate in (cur, *cur.parents):
        if (candidate / ".git").exists():
            return candidate
    return None


def _echo_task_class_and_identity(manifest):
    """Extracts the two fields a schema-invalid manifest can still safely echo back (used by both
    cmd_verify's and cmd_authorize's schema-violation branches) -- only values that themselves
    satisfy the OUTPUT schema's typing are returned; anything else becomes None rather than making
    the error envelope itself schema-invalid."""
    task_class_value = manifest.get("task_class") if isinstance(manifest, dict) else None
    identity_value = manifest.get("identity") if isinstance(manifest, dict) else None
    task_class_echo = task_class_value if isinstance(task_class_value, str) else None
    identity_echo = identity_value if isinstance(identity_value, dict) else None
    return task_class_echo, identity_echo


def _resolve_repo_root(args: argparse.Namespace, on_not_found) -> Path:
    """Shared repo-root resolution for both `verify` and `authorize`: explicit --repo-root
    trusts the caller directly (no .git requirement -- needed for hermetic clean-checkout
    testing, where a `git checkout-index` export has no .git); otherwise auto-detect from this
    script's own location. on_not_found(code, message) is called (and expected to raise
    SystemExit, via the caller's own output-schema-shaped emit) when auto-detection fails."""
    if args.repo_root:
        return Path(args.repo_root).resolve()
    found = _find_repo_root(Path(__file__))
    if found is None:
        on_not_found("REPO_ROOT_NOT_FOUND",
                      "cannot locate repo root (.git not found) from script location; "
                      "pass --repo-root explicitly")
        raise AssertionError("on_not_found must raise SystemExit")  # pragma: no cover -- defensive only
    return found


_UTC_TIMESTAMP_RE = re.compile(
    r'^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,6})?Z$'
)


def _is_valid_utc_timestamp(value) -> bool:
    """Deterministic UTC-only ISO8601 check: exact `YYYY-MM-DDTHH:MM:SS[.ffffff]Z` form (no other
    timezone offset accepted -- 'valid UTC', not merely 'valid ISO8601') plus a real calendar-date
    check (rejects e.g. 2026-02-30) -- a hand-rolled regex + datetime() construction rather than
    `datetime.fromisoformat`/a dependency, matching this file's existing preference for
    deterministic, dependency-free parsers (e.g. the Markdown link scanner) over library quirks."""
    if not isinstance(value, str):
        return False
    m = _UTC_TIMESTAMP_RE.match(value)
    if not m:
        return False
    year, month, day, hour, minute, second = (int(g) for g in m.groups())
    try:
        datetime.datetime(year, month, day, hour, minute, second)
    except ValueError:
        return False
    return True


# =============================================================================
# `authorize` subcommand (Phase 3, plan_26072506 vertical slice 1) -- side-effect-authorization
# output schema (SIDE_EFFECT_SCHEMA), entirely separate from `verify`'s completion-manifest output.
# =============================================================================

def _authorize_bare_result(mode, action, code: str, message: str) -> dict:
    return {
        "schema_version": SCHEMA_VERSION, "mode": mode, "action": action, "task_class": None,
        "authorization_state": None, "allowed": False,
        "reason_codes": [code], "messages": {code: message}, "identity": None,
    }


def _emit_authorization(obj: dict, exit_code: int) -> None:
    obj.setdefault("messages", {})
    _emit_with_schema(obj, exit_code, SIDE_EFFECT_SCHEMA)


# Maps resolve_and_stat_evidence's status vocabulary (also used for evidence.*.path in `verify`)
# onto authorize's EXECUTION_APPROVAL_PLAN_PATH_* reason codes -- "ok" never appears here (the
# caller branches on status == "ok" separately, before consulting this table).
_PLAN_PATH_STATUS_TO_REASON_CODE = {
    "repo_escape": "EXECUTION_APPROVAL_PLAN_PATH_ESCAPES_REPO",
    "symlink": "EXECUTION_APPROVAL_PLAN_PATH_CONTAINS_SYMLINK",
    "not_found": "EXECUTION_APPROVAL_PLAN_PATH_NOT_FOUND",
    "not_a_directory": "EXECUTION_APPROVAL_PLAN_PATH_NOT_FOUND",
    "wrong_type": "EXECUTION_APPROVAL_PLAN_PATH_WRONG_TYPE",
    "os_error": "EXECUTION_APPROVAL_PLAN_PATH_UNREADABLE",
}


def _cmd_authorize_experimental(mode: str, action: str, manifest_path: Path, repo_root: Path) -> None:
    try:
        text = _read_manifest_text(manifest_path)
        manifest = _parse_manifest_json(text)
    except _ManifestLoadError as e:
        _emit_authorization(_authorize_bare_result(mode, action, e.code, e.message), 2)

    shape_violations = validate_against_schema(manifest, WORK_SCHEMA)
    if shape_violations:
        task_class_echo, identity_echo = _echo_task_class_and_identity(manifest)
        _emit_authorization({
            "schema_version": SCHEMA_VERSION, "mode": mode, "action": action,
            "task_class": task_class_echo, "authorization_state": None, "allowed": False,
            "reason_codes": sorted(code for code, _ in shape_violations),
            "messages": {code: msg for code, msg in shape_violations},
            "identity": identity_echo,
        }, 2)

    # From here `manifest` is guaranteed to conform to work-manifest.schema.json -- every direct
    # key/index access below is safe (same exhaustiveness argument as cmd_verify).
    task_class = manifest["task_class"]
    identity = manifest["identity"]
    evidence = manifest.get("evidence") or {}
    execution_approval = manifest.get("execution_approval")

    reason_codes: list[str] = []
    messages: dict[str, str] = {}

    def add_reason(code: str, message: str) -> None:
        reason_codes.append(code)
        messages[code] = message

    def fail(exit_code: int) -> None:
        _emit_authorization({
            "schema_version": SCHEMA_VERSION, "mode": mode, "action": action,
            "task_class": task_class, "authorization_state": None, "allowed": False,
            "reason_codes": sorted(reason_codes), "messages": messages,
            "identity": identity,
        }, exit_code)

    if execution_approval is None:
        add_reason("EXECUTION_APPROVAL_ABSENT", "manifest has no execution_approval block")
        fail(1)

    # Every required key below is guaranteed present by the schema-shape pass above (execution_
    # approval, when present at all, is fully-formed per executionApproval's own `required`).
    plan_path_str = execution_approval["plan_path"]

    if _is_absolute_path_string(plan_path_str):
        add_reason("EXECUTION_APPROVAL_PLAN_PATH_ABSOLUTE",
                   f"execution_approval.plan_path {plan_path_str!r} must be relative to the "
                   f"manifest, not absolute")
        fail(2)

    approved_at_utc = execution_approval["approved_at_utc"]
    if not _is_valid_utc_timestamp(approved_at_utc):
        add_reason("EXECUTION_APPROVAL_TIMESTAMP_INVALID",
                   f"execution_approval.approved_at_utc {approved_at_utc!r} is not a valid UTC "
                   f"timestamp (expected YYYY-MM-DDTHH:MM:SS[.ffffff]Z)")
        fail(2)

    plan_evidence = evidence.get("plan")
    if plan_evidence and plan_evidence.get("path") and plan_evidence["path"] != plan_path_str:
        add_reason("EXECUTION_APPROVAL_PLAN_PATH_MISMATCH",
                   f"execution_approval.plan_path {plan_path_str!r} does not match "
                   f"evidence.plan.path {plan_evidence['path']!r}")
        fail(2)

    repo_root_fd = os.open(str(repo_root), os.O_RDONLY | os.O_DIRECTORY)
    try:
        manifest_dir = manifest_path.resolve().parent
        r = resolve_and_stat_evidence(repo_root_fd, repo_root, manifest_dir, plan_path_str,
                                      expect_dir=False, capture_content=True)
        if r["status"] != "ok":
            code = _PLAN_PATH_STATUS_TO_REASON_CODE.get(r["status"], "EXECUTION_APPROVAL_PLAN_PATH_UNREADABLE")
            add_reason(code,
                       f"execution_approval.plan_path {plan_path_str!r} could not be resolved as a "
                       f"safe, existing, non-empty regular file (status={r['status']})")
            fail(2)
    finally:
        os.close(repo_root_fd)

    if r["sha256"] != execution_approval["plan_sha256"]:
        add_reason("EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH",
                   "execution_approval.plan_sha256 does not match the resolved plan bytes")
        fail(2)
    expected_atoms = [
        f"approved_by: {execution_approval['approved_by']}",
        f"approved_at_utc: {execution_approval['approved_at_utc']}",
        *[f"allowed_action: {value}" for value in execution_approval["allowed_actions"]],
    ]
    if execution_approval["approval_atoms"] != expected_atoms:
        add_reason("EXECUTION_APPROVAL_ATOMS_INVALID",
                   "approval_atoms must exactly equal the deterministic atoms derived from approver, time, and actions")
        fail(2)
    # 2026-09-03(plan_26090317 P3): `plan_bytes` 가 어디에도 정의돼 있지 않아 이 줄이 NameError 로
    #   죽었다 — 즉 **execution_approval 인가 경로는 한 번도 성공한 적이 없다**(B0 가 "안내대로 하면
    #   거부된다" 로 관측한 것의 더 깊은 층: 안내를 고쳐도 그 다음 줄에서 죽었다). resolver 가
    #   capture_content=True 로 이미 읽어 둔 바이트를 쓴다.
    plan_bytes = r.get("content_bytes")
    if not isinstance(plan_bytes, (bytes, bytearray)):
        add_reason("EXECUTION_APPROVAL_PLAN_PATH_UNREADABLE",
                   "resolved plan produced no content bytes to verify the approval anchor against")
        fail(2)
    try:
        plan_lines = plan_bytes.decode("utf-8").splitlines()
    except UnicodeDecodeError:
        add_reason("EXECUTION_APPROVAL_PLAN_UTF8_INVALID", "execution approval plan must be valid UTF-8")
        fail(2)
    required_lines = [execution_approval["approval_anchor"], *expected_atoms]
    if any(line not in plan_lines for line in required_lines):
        add_reason("EXECUTION_APPROVAL_PLAN_ATOMS_MISSING",
                   "resolved plan does not contain the exact approval anchor and derived approval atoms")
        fail(2)

    if execution_approval["approved"] is not True:
        add_reason("EXECUTION_APPROVAL_NOT_APPROVED", "execution_approval.approved is not true")
        fail(1)

    if action not in execution_approval["allowed_actions"]:
        add_reason("EXECUTION_APPROVAL_ACTION_NOT_ALLOWED",
                   f"action {action!r} is not in execution_approval.allowed_actions "
                   f"{execution_approval['allowed_actions']!r}")
        fail(1)

    _emit_authorization({
        "schema_version": SCHEMA_VERSION, "mode": mode, "action": action,
        "task_class": task_class, "authorization_state": "execution-approved", "allowed": True,
        "reason_codes": [], "messages": {},
        "identity": identity,
    }, 0)


def _cmd_authorize_promotion(mode: str, action: str, manifest_path: Path, repo_root: Path) -> None:
    """Reuses the real `verify` evaluation for promotion decisions -- runs it as a real subprocess
    (this same script, `verify` subcommand) rather than re-deriving/duplicating its heavily-
    reviewed state-machine logic in a second code path. No execution_approval is consulted or
    required; the only question is whether verify itself reached state == 'promotion-ready' with
    eligible_for_promotion == true. This function has no side effects of its own beyond spawning
    that read-only subprocess -- a denied/invalid result here causes no filesystem mutation."""
    verify_cmd = [sys.executable, str(Path(__file__).resolve()), "verify",
                  "--manifest", str(manifest_path), "--repo-root", str(repo_root)]
    try:
        proc = subprocess.run(verify_cmd, capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.TimeoutExpired) as e:
        _emit_authorization(_authorize_bare_result(
            mode, action, "PROMOTION_GATE_VERIFY_UNAVAILABLE",
            f"could not execute internal verify evaluation: {e}"), 2)

    try:
        verify_out = json.loads(proc.stdout)
    except json.JSONDecodeError:
        _emit_authorization(_authorize_bare_result(
            mode, action, "PROMOTION_GATE_VERIFY_UNREADABLE",
            f"internal verify subprocess (exit={proc.returncode}) did not produce parseable JSON "
            f"stdout; stderr={proc.stderr.strip()[:500]!r}"), 2)

    task_class_echo = verify_out.get("task_class") if isinstance(verify_out.get("task_class"), str) else None
    identity_echo = verify_out.get("identity") if isinstance(verify_out.get("identity"), dict) else None
    verify_reason_codes = [c for c in (verify_out.get("reason_codes") or []) if isinstance(c, str)]
    verify_messages = verify_out.get("messages") if isinstance(verify_out.get("messages"), dict) else {}

    eligible = (
        proc.returncode == 0
        and verify_out.get("exit_code") == 0
        and verify_out.get("state") == "promotion-ready"
        and verify_out.get("eligible_for_promotion") is True
    )

    if eligible:
        _emit_authorization({
            "schema_version": SCHEMA_VERSION, "mode": mode, "action": action,
            "task_class": task_class_echo, "authorization_state": "promotion-ready", "allowed": True,
            "reason_codes": [], "messages": {},
            "identity": identity_echo,
        }, 0)

    invalid_input = proc.returncode == 2 or verify_out.get("exit_code") == 2
    wrap_code = "PROMOTION_GATE_INVALID_MANIFEST" if invalid_input else "PROMOTION_GATE_NOT_ELIGIBLE"
    wrap_message = (
        "underlying verify evaluation reported invalid input -- see other reason codes for detail"
        if invalid_input else
        f"underlying verify state={verify_out.get('state')!r} eligible_for_promotion="
        f"{verify_out.get('eligible_for_promotion')!r} -- promotion side effects blocked"
    )
    messages = dict(verify_messages)
    messages[wrap_code] = wrap_message
    _emit_authorization({
        "schema_version": SCHEMA_VERSION, "mode": mode, "action": action,
        "task_class": task_class_echo, "authorization_state": None, "allowed": False,
        "reason_codes": sorted(set(verify_reason_codes) | {wrap_code}),
        "messages": messages,
        "identity": identity_echo,
    }, 2 if invalid_input else 1)


def _authorize_tripwire_failure(repo_root: Path) -> str | None:
    """부 실행자 — `authorize` 병목에서 tripwire 3종을 돌린다(실패 사유 문자열 또는 None).

    ⚠ stdout 계약: 이 함수는 **절대 stdout 에 쓰지 않는다**. `authorize` 의 JSON stdout 을
    파싱하는 소비자가 셋이다 — `sync_branches.sh` · `hint_tag.py` · **`sync_to_sub.sh`**
    (`authorize --action sync_to_sub`). 진단 한 줄이라도 새면 셋이 한꺼번에 깨진다.
    자식 프로세스의 stdout/stderr 는 캡처해 삼키고, 사유는 반환값으로만 돌려준다.

    격리 픽스처 제외: `runtime_selftest.py` 가 이 파일 옆에 없거나(픽스처 레포는 completion_gate
    만 복사한다) `--repo-root` 가 이 파일이 속한 저장소가 아니면(clean-checkout export) 건너뛴다.
    runtime_selftest 안에서도 `_is_canonical_repo()` 가 한 번 더 판별한다(이중 방어).

    ★ 2026-09-03 (적대검증 MAJOR ①): 두 제외 경로는 `return None` 으로 **조용히** 빠져나갔고,
    호출부는 그것을 "tripwire 가 통과했다"와 구분하지 못했다. 이제 제외될 때마다 **사유가 담긴
    SKIPPED 한 줄을 stderr 로** 낸다. 제외 자체는 그대로 정상 경로다 — 바뀐 것은 가시성뿐이다.
    ⚠ stdout 은 절대 건드리지 않는다(`sync_branches.sh`·`sync_to_sub.sh`·`hint_tag.py` 셋이
    이 명령의 stdout 을 `json.loads` 한다). 자식의 stdout/stderr 도 그대로 삼킨다.
    """
    selftest = Path(__file__).resolve().parent / "runtime_selftest.py"
    if not selftest.is_file():
        print(f"[authorize] tripwire SKIPPED (runtime_selftest.py absent beside {Path(__file__).name} "
              f"— isolated fixture copy); pre-side-effect tripwires NOT run", file=sys.stderr)
        return None
    if repo_root.resolve() != Path(__file__).resolve().parents[3]:
        print(f"[authorize] tripwire SKIPPED (--repo-root {repo_root} is not this file's repo "
              f"{Path(__file__).resolve().parents[3]} — clean-checkout export); "
              f"pre-side-effect tripwires NOT run", file=sys.stderr)
        return None
    proc = subprocess.run([sys.executable, "-B", str(selftest), "--tripwires-only"],
                          cwd=str(repo_root), capture_output=True, text=True, timeout=120)
    if proc.returncode == 0:
        return None
    return (proc.stderr or proc.stdout).strip().splitlines()[-1] if (proc.stderr or proc.stdout) \
        else f"runtime_selftest --tripwires-only exited {proc.returncode}"


def cmd_authorize(args: argparse.Namespace) -> None:
    mode = args.mode
    action = args.action
    manifest_path = Path(args.manifest)

    def _on_repo_root_not_found(code: str, message: str) -> None:
        _emit_authorization(_authorize_bare_result(mode, action, code, message), 2)

    repo_root = _resolve_repo_root(args, _on_repo_root_not_found)

    tripwire_failure = _authorize_tripwire_failure(repo_root)
    if tripwire_failure is not None:
        _emit_authorization(_authorize_bare_result(
            mode, action, "TRIPWIRE_FAILED",
            f"pre-side-effect tripwire failed: {tripwire_failure}"), 2)

    if mode == "promotion":
        _cmd_authorize_promotion(mode, action, manifest_path, repo_root)
    else:
        _cmd_authorize_experimental(mode, action, manifest_path, repo_root)


def cmd_verify(args: argparse.Namespace) -> None:
    manifest_path = Path(args.manifest)

    if args.repo_root:
        # explicit override trusts the caller directly (no .git requirement) -- needed for
        # hermetic clean-checkout testing (E3), where a `git checkout-index` export has no .git.
        repo_root = Path(args.repo_root).resolve()
    else:
        found = _find_repo_root(Path(__file__))
        if found is None:
            _emit(_bare_result("REPO_ROOT_NOT_FOUND",
                               "cannot locate repo root (.git not found) from script location; "
                               "pass --repo-root explicitly"), 2)
        repo_root = found

    manifest = _load_manifest(manifest_path)

    # ---- schema-shape validation (invalid-input class, evaluated before ANY operational logic
    # touches the manifest -- a manifest that doesn't match work-manifest.schema.json's declared
    # shape can't be meaningfully evaluated, and this generic pass makes malformed nested types
    # (root not an object, runtime="healthy", pii_scan="yes", ...) impossible to reach the
    # `.get()`-calling code below at all, rather than trying to catch them with a blanket
    # try/except that would also swallow genuine programmer bugs). ----
    shape_violations = validate_against_schema(manifest, WORK_SCHEMA)
    if shape_violations:
        task_class_value = manifest.get("task_class") if isinstance(manifest, dict) else None
        identity_value = manifest.get("identity") if isinstance(manifest, dict) else None
        # Echo only values that can themselves satisfy the completion-output schema. Invalid input
        # types are represented by their stable reason codes; copying them verbatim would make the
        # error envelope schema-invalid and turn an expected exit-2 response into an AssertionError.
        task_class_echo = task_class_value if isinstance(task_class_value, str) else None
        identity_echo = identity_value if isinstance(identity_value, dict) else None
        _emit({
            "schema_version": SCHEMA_VERSION, "task_class": task_class_echo, "state": None,
            "eligible_for_promotion": False,
            "reason_codes": sorted(code for code, _ in shape_violations),
            "messages": {code: msg for code, msg in shape_violations},
            "checked_evidence": {}, "identity": identity_echo,
        }, 2)

    # From here on `manifest` is guaranteed to conform to work-manifest.schema.json -- every
    # `.get()` below is safe because the generic validator already rejected any shape it doesn't
    # expect (this is the exhaustiveness that lets business logic skip defensive try/except).
    task_class = manifest["task_class"]
    identity = manifest["identity"]
    evidence = manifest.get("evidence") or {}
    runtime = manifest.get("runtime")
    capacity_rejection = manifest.get("capacity_rejection")
    benchmark = manifest.get("benchmark") or {}
    conditions = manifest.get("conditions") or {}
    pii_scan = manifest["pii_scan"]
    verdict = benchmark.get("verdict")
    mode = benchmark.get("mode")

    reason_codes: list[str] = []
    messages: dict[str, str] = {}
    checked_evidence: dict[str, dict] = {}

    def add_reason(code: str, message: str) -> None:
        reason_codes.append(code)
        messages[code] = message

    # ---- absolute-path input-contract check (ABSOLUTE_EVIDENCE_PATHS_ACCEPTED fix): every
    # evidence.*.path, capacity_rejection.gate_evidence, and pii_scan.scanned_paths entry is
    # documented (work-manifest.schema.json) as relative to the manifest's own directory. An
    # absolute string violates that contract regardless of whether it happens to normalize to
    # somewhere still inside the repo -- checked on the RAW string, BEFORE _lexical_components
    # ever joins/normalizes it (pathlib's `manifest_dir / abs_path` silently discards
    # manifest_dir entirely, which is exactly how an absolute-but-in-repo path could otherwise
    # slip past the repo-escape check and resolve as if nothing were wrong). ----
    absolute_violations: list[tuple[str, str]] = []
    for key, item in evidence.items():
        path_str = (item or {}).get("path")
        if path_str and _is_absolute_path_string(path_str):
            absolute_violations.append((f"ABSOLUTE_EVIDENCE_PATH:{key}",
                                        f"evidence '{key}' path {path_str!r} must be relative to the manifest, not absolute"))
    if capacity_rejection and capacity_rejection.get("gate_evidence"):
        gate_path_str = capacity_rejection["gate_evidence"]
        if _is_absolute_path_string(gate_path_str):
            absolute_violations.append(("ABSOLUTE_CAPACITY_GATE_EVIDENCE_PATH",
                                        f"capacity_rejection.gate_evidence path {gate_path_str!r} must be relative to the manifest, not absolute"))
    for idx, scanned_path in enumerate(pii_scan.get("scanned_paths") or []):
        if _is_absolute_path_string(scanned_path):
            absolute_violations.append((f"ABSOLUTE_PII_SCANNED_PATH:{idx}",
                                        f"pii_scan.scanned_paths[{idx}] {scanned_path!r} must be relative to the manifest, not absolute"))
    if absolute_violations:
        for code, msg in absolute_violations:
            add_reason(code, msg)
        _emit({
            "schema_version": SCHEMA_VERSION, "task_class": task_class, "state": None,
            "eligible_for_promotion": False,
            "reason_codes": sorted(reason_codes), "messages": messages,
            "checked_evidence": checked_evidence, "identity": identity,
        }, 2)

    # ---- pre-state-machine structural contract (invalid-input class): a certificate is only
    # ever published PASS-side (adversarial-benchmark contract: publish_benchmark_record.py is a
    # no-op on non-PASS). A manifest claiming a certificate while its own benchmark.verdict is
    # anything other than the literal string "PASS" -- including null/absent -- is
    # self-contradictory, not merely "incomplete evidence". PASS_ONLY_CERTIFICATE_GAP fix:
    # covers null/absent verdict too, not just an explicit non-PASS string. ----
    if evidence.get("certificate") and verdict != "PASS":
        add_reason("CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT",
                   f"benchmark.verdict={verdict!r} but evidence.certificate is present "
                   f"(certificates are published PASS-only)")
        _emit({
            "schema_version": SCHEMA_VERSION, "task_class": task_class, "state": None,
            "eligible_for_promotion": False,
            "reason_codes": sorted(reason_codes), "messages": messages,
            "checked_evidence": checked_evidence, "identity": identity,
        }, 2)

    try:
        repo_root_fd = os.open(str(repo_root), os.O_RDONLY | os.O_DIRECTORY)
    except OSError as e:
        _emit(_bare_result("REPO_ROOT_UNREADABLE", f"could not open repo root {repo_root}: {e}"), 2)

    try:
        manifest_dir = manifest_path.resolve().parent

        # required_keys computed once, up front -- reused for the resolution pass (which paths to
        # capture full content for), the evidence-completeness loop, and PII coverage.
        required_keys = required_evidence_for(task_class, conditions, verdict)

        # ---- upfront hardened resolution pass over EVERY path-bearing field (evidence.* and
        # capacity_rejection.gate_evidence) BEFORE any state-machine tier decision. This is what
        # fixes CAPACITY_PATH_ESCAPE_MISCLASSIFIED: an escaping capacity-gate path is now
        # classified identically to an escaping evidence path (exit 2, explicit code), never
        # silently downgraded into PRE_RUNTIME_NOT_HEALTHY/exit 1 because it happened to be
        # evaluated inside the runtime tier's own logic. ----
        all_evidence_keys = list(dict.fromkeys([*required_keys, *evidence.keys()]))

        resolved: dict[str, dict | None] = {}
        escape_violations: list[tuple[str, str]] = []
        for key in all_evidence_keys:
            item = evidence.get(key)
            if not item or not item.get("path"):
                resolved[key] = None
                continue
            # certificates are parsed (need full bytes); required Markdown-like evidence needs
            # full bytes too so its local links can be scanned from the SAME hashed content
            # (REQUIRED_LINK_VALIDATION_ABSENT fix) -- never a second, separately-opened read.
            need_content = key == "certificate" or (key in MARKDOWN_LIKE_EVIDENCE_KEYS and key in required_keys)
            r = resolve_and_stat_evidence(repo_root_fd, repo_root, manifest_dir, item["path"],
                                           expect_dir=(key in DIR_EVIDENCE_KEYS),
                                           capture_content=need_content)
            resolved[key] = r
            if r["status"] == "repo_escape":
                escape_violations.append((f"EVIDENCE_PATH_ESCAPES_REPO:{key}",
                                          f"evidence '{key}' path {item['path']!r} escapes the repo root via '..' traversal"))
            elif r["status"] == "symlink":
                escape_violations.append((f"EVIDENCE_PATH_CONTAINS_SYMLINK:{key}",
                                          f"evidence '{key}' path {item['path']!r} contains a symlink at some path component"))

        cr_gate_resolved = None
        if capacity_rejection and capacity_rejection.get("gate_evidence"):
            cr_gate_resolved = resolve_and_stat_evidence(repo_root_fd, repo_root, manifest_dir,
                                                          capacity_rejection["gate_evidence"], expect_dir=False)
            if cr_gate_resolved["status"] == "repo_escape":
                escape_violations.append(("EVIDENCE_PATH_ESCAPES_REPO:capacity_rejection.gate_evidence",
                                          f"capacity_rejection.gate_evidence path {capacity_rejection['gate_evidence']!r} "
                                          f"escapes the repo root via '..' traversal"))
            elif cr_gate_resolved["status"] == "symlink":
                escape_violations.append(("EVIDENCE_PATH_CONTAINS_SYMLINK:capacity_rejection.gate_evidence",
                                          f"capacity_rejection.gate_evidence path {capacity_rejection['gate_evidence']!r} "
                                          f"contains a symlink at some path component"))

        if escape_violations:
            for code, msg in escape_violations:
                add_reason(code, msg)
            _emit({
                "schema_version": SCHEMA_VERSION, "task_class": task_class, "state": None,
                "eligible_for_promotion": False,
                "reason_codes": sorted(reason_codes), "messages": messages,
                "checked_evidence": checked_evidence, "identity": identity,
            }, 2)

        # capacity_rejection.gate_evidence is required raw evidence for that task_class -- surface
        # it in checked_evidence too (CAPACITY_GATE_RAW_PII_COVERAGE_GAP fix, checked-evidence half
        # -- it used to be entirely invisible in the output, unlike every evidence.* entry).
        if capacity_rejection is not None:
            checked_evidence["capacity_rejection.gate_evidence"] = {
                "required": task_class == "capacity_rejection",
                "path": capacity_rejection.get("gate_evidence"),
                "exists": bool(cr_gate_resolved and cr_gate_resolved["status"] == "ok"),
                "type": (cr_gate_resolved["kind"] if cr_gate_resolved else None),
                "sha256": (cr_gate_resolved["sha256"] if cr_gate_resolved else None),
                "tree_sha256": (cr_gate_resolved["tree_sha256"] if cr_gate_resolved else None),
                "identity_checked": False, "identity_match": None,
            }

        # ---- runtime tier ----
        # capacity_rejection substitutes for a live runtime: rejection happens *before* any
        # container starts, by design (plan_26072506 §4.3), so a valid, hardened-resolved
        # gate_evidence path stands in for "runtime tier satisfied".
        runtime_ok = False
        if task_class == "capacity_rejection":
            capacity_rejection_ok = bool(
                capacity_rejection and capacity_rejection.get("rejected") is True
                and cr_gate_resolved is not None and cr_gate_resolved["status"] == "ok"
            )
            runtime_ok = capacity_rejection_ok
            if not runtime_ok:
                add_reason("PRE_RUNTIME_NOT_HEALTHY",
                           "no valid capacity_rejection substitute (rejected != true, or gate_evidence missing/unresolvable)")
        else:
            # B2: runtime-backed classes require health_ok, no oom-killed container,
            # functional_smoke_passed=true, AND runtime.identity matching all 6 strong fields.
            if not runtime or runtime.get("health_ok") is not True:
                add_reason("PRE_RUNTIME_NOT_HEALTHY", "runtime.health_ok is not true")
            elif any(c.get("oom_killed") for c in (runtime.get("containers") or [])):
                add_reason("PRE_RUNTIME_NOT_HEALTHY", "a runtime container was oom_killed")
            elif runtime.get("functional_smoke_passed") is not True:
                add_reason("RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED", "runtime.functional_smoke_passed is not true")
            else:
                rt_identity = runtime.get("identity")
                if not isinstance(rt_identity, dict):
                    add_reason("RUNTIME_IDENTITY_MISSING", "runtime.identity is absent")
                else:
                    mismatched = [f for f in STRONG_IDENTITY_FIELDS if rt_identity.get(f) != identity.get(f)]
                    if mismatched:
                        for f in mismatched:
                            add_reason(f"RUNTIME_IDENTITY_MISMATCH:{f}",
                                       f"runtime.identity.{f}={rt_identity.get(f)!r} != manifest identity.{f}={identity.get(f)!r}")
                    else:
                        runtime_ok = True

        if not runtime_ok:
            _emit({
                "schema_version": SCHEMA_VERSION, "task_class": task_class, "state": None,
                "eligible_for_promotion": False,
                "reason_codes": sorted(reason_codes), "messages": messages,
                "checked_evidence": checked_evidence, "identity": identity,
            }, 1)

        # ---- evidence tier (evidence-complete computation) ----
        all_present = True
        identity_ok = True
        links_ok = True
        certificate_output: dict | None = None
        # rubric contract outcome, carried to the promotion tier (plan_26082219 D5/U1)
        cert_rubric_ok = False
        cert_rubric_authority: str | None = None

        for key in all_evidence_keys:
            item = evidence.get(key)
            required = key in required_keys
            r = resolved.get(key)
            exists = bool(r and r["status"] == "ok")
            identity_checked = False
            identity_match = None

            if key == "certificate" and exists:
                identity_checked = True
                content_bytes = r.get("content_bytes") or b""
                try:
                    cert_text = content_bytes.decode("utf-8")
                    cert_fields, parse_ok = parse_flat_certificate(cert_text)
                except UnicodeDecodeError:
                    cert_fields, parse_ok = {}, False
                if not parse_ok:
                    identity_ok = False
                    identity_match = False
                    add_reason("CERTIFICATE_UNPARSEABLE",
                               f"certificate at {item['path']!r} is not in the recognized flat-scalar format")
                    certificate_output = {"path": item["path"], "parsed": False, "identity": None,
                                          "verdict": None, "benchmark_mode": None, "identity_match": None,
                                          "rubric_authority": None, "floor_tps": None,
                                          "ratio_M_over_primary": None, "primary_source": None,
                                          "rubric_contract_ok": False,
                                          "run_key": None, "duplicates": []}
                else:
                    mapped, mismatched = _certificate_strong_identity_matches(cert_fields, identity)
                    identity_match = not mismatched
                    if mismatched:
                        identity_ok = False
                        for f in mismatched:
                            add_reason(f"IDENTITY_MISMATCH:{f}",
                                       f"certificate {CERTIFICATE_FIELD_MAP[f]}={mapped.get(f)!r} "
                                       f"!= manifest identity.{f}={identity.get(f)!r}")
                    # ---- 측정 키 해소 (plan_26090410 §3.2) — 이 인증서가 증명하는 "측정 하나" 가
                    #      식별되는가(0), 그리고 같은 측정을 증명하는 **다른 추적 파일**이 있는가(2+).
                    #      2+ 는 단일 권위 위반(같은 사실이 두 자리에)이므로 승격을 막는다. 바이트가
                    #      같으면 중복층(하나를 지워라), 다르면 더 나쁜 결함(같은 측정 다른 내용)이다.
                    run_key = certificate_run_key(cert_fields)
                    duplicates: list[dict] = []
                    if run_key is None:
                        identity_ok = False
                        add_reason("CERTIFICATE_RUN_KEY_UNRESOLVABLE",
                                   f"certificate at {item['path']!r} lacks a strong identity field or "
                                   f"measured_utc -- the measurement it certifies cannot be identified (fail-closed)")
                    else:
                        # manifest 가 선언한 측정(benchmark.measured_utc · 인증서에서 파생)과 파일이 갈라지면
                        # 이 manifest 는 다른 런의 인증서를 가리키고 있다(감사 B-1: 6키만으론 런을 못 가른다).
                        declared_utc = (benchmark.get("measured_utc") or None) if isinstance(benchmark, dict) else None
                        if declared_utc and declared_utc != run_key[-1]:
                            identity_ok = False
                            add_reason("CERTIFICATE_RUN_DRIFT",
                                       f"manifest benchmark.measured_utc={declared_utc!r} != certificate "
                                       f"measured_utc={run_key[-1]!r} -- the manifest binds a different measurement "
                                       f"than the file at {item['path']!r} certifies")
                        duplicates = _sibling_certificates_with_key(repo_root_fd, repo_root, manifest_dir,
                                                                    item["path"], run_key)
                        if duplicates:
                            identity_ok = False
                            same_bytes = all(d["sha256"] == r["sha256"] for d in duplicates)
                            add_reason("CERTIFICATE_RUN_AMBIGUOUS",
                                       f"measurement {run_key[0]}@{run_key[-1]} is certified by "
                                       f"{len(duplicates) + 1} tracked files ({item['path']!r} + "
                                       f"{[d['path'] for d in duplicates]}) -- "
                                       + ("byte-identical: duplicate layer, delete all but the original"
                                          if same_bytes else
                                          "DIFFERENT bytes for the same measurement: publication-chain defect"))
                    cert_verdict = cert_fields.get("verdict")
                    cert_mode = cert_fields.get("benchmark_mode")
                    if cert_verdict != "PASS":
                        identity_ok = False
                        add_reason("CERTIFICATE_VERDICT_MISMATCH",
                                   f"certificate verdict={cert_verdict!r} (must be 'PASS')")
                    if cert_mode != "full":
                        identity_ok = False
                        add_reason("CERTIFICATE_BENCHMARK_MODE_MISMATCH",
                                   f"certificate benchmark_mode={cert_mode!r} (must be 'full')")
                    # ---- rubric contract (plan_26082219 D5) -- fail-closed on the ARTIFACT ----
                    floor_tps = _certificate_number(cert_fields.get("floor_tps"))
                    ratio_value = _certificate_number(cert_fields.get("ratio_M_over_primary"))
                    primary_source = (cert_fields.get("primary_source") or "").strip()
                    raw_authority = (cert_fields.get("rubric_authority") or "").strip()
                    cert_rubric_ok = True
                    if floor_tps is None or floor_tps <= 0:
                        cert_rubric_ok = False
                        identity_ok = False
                        add_reason("CERTIFICATE_RUBRIC_FLOOR_INVALID",
                                   f"certificate floor_tps={cert_fields.get('floor_tps')!r} is not a finite "
                                   f"positive number -- floor<=0 means the rubric imposed NO threshold "
                                   f"(공허 PASS): every measurement passes. Not promotable in ANY authority.")
                    if ratio_value is None:
                        cert_rubric_ok = False
                        identity_ok = False
                        add_reason("CERTIFICATE_RUBRIC_RATIO_MISSING",
                                   f"certificate ratio_M_over_primary={cert_fields.get('ratio_M_over_primary')!r} "
                                   f"is not a finite number -- the measurement-vs-rubric indicator is absent, "
                                   f"so the PASS cannot be audited")
                    if not primary_source or primary_source.upper() == "N/A":
                        cert_rubric_ok = False
                        identity_ok = False
                        add_reason("CERTIFICATE_RUBRIC_SOURCE_MISSING",
                                   f"certificate primary_source={cert_fields.get('primary_source')!r} is absent "
                                   f"-- which rubric slot won cannot be established (surfaced, never matched "
                                   f"as a string: the numeric floor/ratio contract above is the gate)")
                    # authority: legacy tolerance. Certificates published before this field existed
                    # (2026-08-22) simply do not carry it -- requiring it would turn every existing
                    # hint tag red because the schema grew, not because the perf fact changed. Only
                    # the VALUE RANGE is gated.
                    if raw_authority and raw_authority.upper() != "N/A":
                        if raw_authority in RUBRIC_AUTHORITIES:
                            cert_rubric_authority = raw_authority
                        else:
                            cert_rubric_ok = False
                            identity_ok = False
                            add_reason("CERTIFICATE_RUBRIC_AUTHORITY_UNKNOWN",
                                       f"certificate rubric_authority={raw_authority!r} is not one of "
                                       f"{list(RUBRIC_AUTHORITIES)} -- an unknown rubric authority cannot be "
                                       f"reasoned about, fail closed")
                    certificate_output = {
                        "path": item["path"], "parsed": True, "identity": mapped,
                        "verdict": cert_verdict, "benchmark_mode": cert_mode, "identity_match": identity_match,
                        # 출처 표시(헌법 §결정론 규율): 판정기가 *무엇을 근거로* 승격을 열었는지 산출물이 밝힌다.
                        "rubric_authority": cert_rubric_authority,
                        "floor_tps": floor_tps, "ratio_M_over_primary": ratio_value,
                        "primary_source": primary_source or None,
                        "rubric_contract_ok": cert_rubric_ok,
                        # 출처 표시: 이 인증서가 식별하는 측정과, 같은 측정을 주장하는 다른 추적 파일들
                        "run_key": (list(run_key) if run_key else None),
                        "duplicates": [d["path"] for d in duplicates],
                    }

            if key in MARKDOWN_LIKE_EVIDENCE_KEYS and required and exists:
                # links are relative to the EVIDENCE FILE's own directory (Markdown convention),
                # not manifest_dir -- re-derive it via the same pure lexical (no I/O) helper
                # already proven safe when `r["status"] == "ok"` was established above.
                lex_status, parts = _lexical_components(manifest_dir, repo_root, item["path"])
                base_dir = repo_root.joinpath(*parts[:-1]) if lex_status == "ok" else manifest_dir
                content_bytes = r.get("content_bytes") or b""
                unreadable, broken, invalid = check_required_local_links(repo_root_fd, repo_root, base_dir, content_bytes)
                if unreadable:
                    links_ok = False
                    add_reason(f"EVIDENCE_LINK_SCAN_FAILED:{key}",
                               f"evidence '{key}' is not valid UTF-8 -- cannot scan for local links, failing closed")
                else:
                    if broken:
                        links_ok = False
                        add_reason(f"EVIDENCE_BROKEN_LINK:{key}",
                                   f"evidence '{key}' contains broken local link target(s): {broken}")
                    if invalid:
                        links_ok = False
                        add_reason(f"EVIDENCE_INVALID_LINK:{key}",
                                   f"evidence '{key}' contains invalid local link target(s): {invalid}")

            checked_evidence[key] = {
                "required": required, "path": (item or {}).get("path"), "exists": exists,
                "type": (r["kind"] if r else None),
                "sha256": (r["sha256"] if r else None),
                "tree_sha256": (r["tree_sha256"] if r else None),
                "identity_checked": identity_checked, "identity_match": identity_match,
            }
            if required and not exists:
                all_present = False
                status_label = r["status"] if r else "absent"
                add_reason(f"EVIDENCE_MISSING:{key}", f"required evidence '{key}' missing/unresolvable (status={status_label})")

        or_group_key = None
        if task_class == "capacity_rejection":
            devlog_exists = bool(resolved.get("devlog") and resolved["devlog"]["status"] == "ok")
            testlog_exists = bool(resolved.get("testlog") and resolved["testlog"]["status"] == "ok")
            if not (devlog_exists or testlog_exists):
                all_present = False
                add_reason("EVIDENCE_MISSING:devlog_or_testlog", "capacity_rejection requires at least one of devlog/testlog")
            or_group_key = "devlog" if devlog_exists else ("testlog" if testlog_exists else None)

        # ---- PII gate (C3): binds to evidence-complete itself, not only promotion. Coverage
        # is checked against every path this task_class actually requires (incl. the
        # capacity_rejection OR-group's satisfying key AND, for capacity_rejection specifically,
        # its own raw gate_evidence path -- CAPACITY_GATE_RAW_PII_COVERAGE_GAP fix). ----
        pii_ok = pii_scan.get("passed") is True
        scanned_paths = set(pii_scan.get("scanned_paths") or [])
        pii_check_keys = list(required_keys) + ([or_group_key] if or_group_key else [])
        pii_coverage_ok = True
        for key in pii_check_keys:
            item = evidence.get(key)
            item_path = (item or {}).get("path")
            if item_path and item_path not in scanned_paths:
                pii_coverage_ok = False
                add_reason(f"PII_SCAN_COVERAGE_INCOMPLETE:{key}",
                           f"required evidence '{key}' path {item_path!r} not present in pii_scan.scanned_paths")
        if task_class == "capacity_rejection" and capacity_rejection and capacity_rejection.get("gate_evidence"):
            gate_path = capacity_rejection["gate_evidence"]
            if gate_path not in scanned_paths:
                pii_coverage_ok = False
                add_reason("PII_SCAN_COVERAGE_INCOMPLETE:capacity_rejection.gate_evidence",
                           f"required capacity_rejection.gate_evidence path {gate_path!r} not present in pii_scan.scanned_paths")
        if not pii_ok:
            add_reason("PII_SCAN_FAILED", "pii_scan.passed is not true -- promotion/evidence-complete side effects blocked")

        state = "evidence-complete" if (all_present and identity_ok and links_ok and pii_ok and pii_coverage_ok) else "runtime-ready"

        if state == "evidence-complete" and task_class in PROMOTION_CAPPED_TASK_CLASSES:
            add_reason(f"TASK_CLASS_CAPS_BELOW_PROMOTION:{task_class}",
                       f"task_class '{task_class}' architecturally cannot reach promotion-ready")

        # ---- rubric authority resolution: certificate first, manifest.benchmark as fallback ----
        # 승격이 *무엇을 근거로* 열렸는지는 산출물이 스스로 밝혀야 한다(헌법 §결정론 규율 출처 표시).
        # 우선순위는 **아티팩트 > 선언**이다: 디스크의 인증서가 있으면 그것이 정본이고, manifest 는
        # 인증서가 구조적으로 존재할 수 없을 때(=verdict!=PASS)만 carrier 가 된다.
        certificate_present = bool(resolved.get("certificate") and resolved["certificate"]["status"] == "ok")
        manifest_rubric = _manifest_rubric_contract(benchmark)
        rubric_authority = cert_rubric_authority
        rubric_ok = cert_rubric_ok
        rubric_floor = (certificate_output or {}).get("floor_tps")
        rubric_ratio = (certificate_output or {}).get("ratio_M_over_primary")
        rubric_primary_source = (certificate_output or {}).get("primary_source")
        rubric_authority_source = "certificate" if certificate_present else None
        if manifest_rubric["declared"]:
            if certificate_present:
                # 두 carrier 가 갈라지면 이 결함 계열(통로 불일치)이 그대로 다시 자란다. 판정은
                # 인증서를 따르되(아티팩트 우선), 갈라진 사실 자체는 **표면화**한다 — 정적 파일끼리는
                # 한쪽이 다른 쪽을 생성할 수 없으므로 교차검증이 차선이다(workflow.md §결정론 규율).
                if manifest_rubric["authority"] and cert_rubric_authority and \
                        manifest_rubric["authority"] != cert_rubric_authority:
                    add_reason(
                        "RUBRIC_AUTHORITY_CARRIER_DISAGREEMENT",
                        f"manifest benchmark.rubric_authority={manifest_rubric['authority']!r} != "
                        f"certificate rubric_authority={cert_rubric_authority!r} -- 인증서(디스크 아티팩트)를 "
                        f"정본으로 판정한다. 두 carrier 가 갈라진 것은 발행 체인의 결함이므로 확인하라.")
            else:
                for code, message in manifest_rubric["problems"]:
                    add_reason(code, message)
                rubric_authority = manifest_rubric["authority"]
                rubric_ok = manifest_rubric["ok"]
                rubric_floor = manifest_rubric["floor_tps"]
                rubric_ratio = manifest_rubric["ratio_M_over_primary"]
                rubric_primary_source = manifest_rubric["primary_source"]
                rubric_authority_source = "manifest_benchmark"
        rubric_output = None
        if certificate_present or manifest_rubric["declared"]:
            rubric_output = {
                "authority": rubric_authority, "authority_source": rubric_authority_source,
                "contract_ok": bool(rubric_ok), "floor_tps": rubric_floor,
                "ratio_M_over_primary": rubric_ratio, "primary_source": rubric_primary_source,
            }

        # ---- promotion tier ----
        # PROMOTION_CERTIFICATE_BYPASS fix: promotion is modeled EXCLUSIVELY by task_class ==
        # "full_benchmark" -- no other class may reach promotion-ready by self-declaring
        # benchmark.mode=full/verdict=PASS, even when otherwise evidence-complete. By the time
        # state == "evidence-complete" for full_benchmark, the certificate (when required) has
        # already been present, parsed, identity-matched, and verdict/mode-checked above -- this
        # tier only needs the manifest-level mode/verdict re-confirmation plus the capped-class gate.
        eligible = False
        if state == "evidence-complete" and task_class not in PROMOTION_CAPPED_TASK_CLASSES:
            if task_class != "full_benchmark":
                add_reason("PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS",
                           f"task_class={task_class!r} can never reach promotion-ready -- only 'full_benchmark' may")
            elif mode != "full":
                add_reason("BENCHMARK_MODE_NOT_FULL", f"benchmark.mode={mode!r} (must be 'full' for promotion)")
            elif verdict != "PASS":
                # ---- explore authority: 성능 판정은 게이트가 아니라 **서술**이다 (plan_26082219 U1) ----
                # 사용자가 HITL 로 *"목표 0/광범위 탐색"* 을 지시한 런에서는 PASS/REFUTE 가 "벽 지도
                # 데이터"이지 통과 조건이 아니다. 그 모드의 승격 자격은 **서빙 성립(기능)** -- 모델이
                # 실제로 떠서 통신에 응답한다 -- **∧ 유효 측정**(floor>0 ∧ ratio 실재)이다.
                # 서빙 성립은 위 runtime tier 가 이미 강제한다(health_ok ∧ oom_killed 없음 ∧
                # functional_smoke_passed ∧ runtime.identity 6키 일치): 그 관문을 통과하지 못하면
                # 여기까지 오지 못한다. 아래 재확인은 **U1 계약을 코드에 명시**하기 위한 것이며,
                # 조건을 완화하지 않는다(runtime tier 가 바뀌어도 이 경로는 스스로 닫힌다).
                # ★ 공허 PASS 배제는 불변이다 -- floor<=0 이면 rubric_ok=False 라 여기서도 못 연다
                #   (인증서 carrier·manifest carrier 둘 다 같은 계약을 통과해야 True 가 된다).
                # ★ carrier 는 인증서가 아니라 **resolve 된 rubric** 이다 — 인증서는 PASS 전용
                #   산출물이라 REFUTE 에서는 구조적으로 부재하고, 인증서에서만 읽으면 이 경로가
                #   REFUTE 에서 영원히 발화하지 못한다(plan_26082405 · 죽은 코드였던 원인).
                functional_smoke_ok = bool(runtime) and runtime.get("functional_smoke_passed") is True
                if rubric_authority == "explore" and rubric_ok and functional_smoke_ok:
                    eligible = True
                    add_reason(
                        "BENCHMARK_EXPLORE_AUTHORITY_PROMOTION",
                        f"benchmark.verdict={verdict!r} + rubric_authority='explore' "
                        f"(사용자 HITL 탐색 트리거 · carrier={rubric_authority_source!r}) → 승격 게이트는 "
                        f"성능 판정이 아니라 **서빙 성립(functional_smoke_passed) + 유효 측정**"
                        f"(floor_tps={rubric_floor!r}, ratio={rubric_ratio!r})이다. "
                        f"PASS/REFUTE 는 벽 지도 데이터로 기록될 뿐 차단하지 않는다.")
                # ---- human-authorized perf waiver (loop-until-done break) ----
                # 통상 REFUTE 는 서빙전략 재수립 + 벤치마커의 측정평면 확장(마지막 평면은 사람이
                # 수동 수집한 정보까지 투입)을 loop-until-done 으로 반복해야 한다. 그 루프는
                # **사람의 지시로만** 중단할 수 있다(2026-08-01 사용자 결정).
                # 그래서 이 예외는 에이전트가 추론으로 열 수 없는 **positive key** 다 —
                # 네 필드가 전부 비어있지 않아야 하고, 하나라도 없으면 종전대로 차단한다.
                # 대가: waiver 가 있으면 배포 산출물에 경고 플래그가 **강제**된다(hint_tag 가 집행).
                # 주의: 이 waiver 는 승격만 연다. 인증서는 여전히 PASS 때만 발행된다
                #       (publish_benchmark_record 무변경) — "성능이 검증됐다"는 주장은 못 만든다.
                waiver = benchmark.get("perf_waiver") if isinstance(benchmark, dict) else None
                fields = ("authorized_by", "authorized_at_utc", "instruction", "warning_flag")
                waiver_ok = (
                    isinstance(waiver, dict)
                    and all(isinstance(waiver.get(k), str) and waiver.get(k).strip() for k in fields)
                )
                if eligible:
                    pass  # explore authority already opened promotion above -- no waiver needed
                elif waiver_ok:
                    eligible = True
                    add_reason(
                        "BENCHMARK_VERDICT_WAIVED",
                        f"benchmark.verdict={verdict!r} + 사람 승인 perf_waiver"
                        f"(by {waiver['authorized_by']!r} at {waiver['authorized_at_utc']!r}) "
                        f"→ 승격 허용. 배포물에 경고 플래그 강제. 인증서는 여전히 미발행.")
                elif waiver is not None:
                    add_reason(
                        "BENCHMARK_PERF_WAIVER_MALFORMED",
                        "perf_waiver 가 있으나 authorized_by/authorized_at_utc/instruction/"
                        "warning_flag 중 비어있는 항목이 있다 — fail-closed 로 차단한다.")
                else:
                    add_reason("BENCHMARK_VERDICT_NOT_PASS", f"benchmark.verdict={verdict!r} (must be 'PASS' for promotion)")
            else:
                eligible = True
                if rubric_authority == "explore":
                    # explore 에서 PASS 는 *우연히 문턱을 넘은 것*이지 게이트 통과가 아니다 --
                    # 승격을 연 실제 근거(서빙 성립 + 유효 측정)를 산출물에 남긴다(출처 표시).
                    add_reason(
                        "BENCHMARK_EXPLORE_AUTHORITY_PROMOTION",
                        f"rubric_authority='explore'(carrier={rubric_authority_source!r}) -- 승격 근거는 "
                        f"성능 판정이 아니라 서빙 성립 + 유효 측정이다(floor_tps={rubric_floor!r}, "
                        f"ratio={rubric_ratio!r}). verdict={verdict!r} 는 벽 지도 데이터로 기록된다.")

        if eligible:
            state = "promotion-ready"

        _emit({
            "schema_version": SCHEMA_VERSION, "task_class": task_class, "state": state,
            "eligible_for_promotion": eligible,
            "reason_codes": sorted(reason_codes), "messages": messages,
            "checked_evidence": checked_evidence, "identity": identity,
            "rubric": rubric_output,
            "certificate": certificate_output,
        }, 0 if eligible else 1)
    finally:
        os.close(repo_root_fd)


class _GateArgumentParser(argparse.ArgumentParser):
    """Overrides argparse's default usage-error handling (prose to stderr + bare exit 2) so that
    malformed CLI invocations (missing subcommand, missing --manifest, unrecognized arguments)
    honor the same stable-JSON-on-stdout / exit-2 contract as manifest/input errors, rather than
    being a second, inconsistent error-reporting path (small contract fix, second review cycle).

    `add_subparsers()` propagates this same class to every subparser (argparse defaults
    `parser_class` to `type(self)`), so `self.prog` distinguishes WHICH subcommand's invocation
    failed ("completion_gate.py authorize" vs "completion_gate.py verify" vs the bare top-level
    "completion_gate.py" for e.g. a missing subcommand entirely) -- routing each usage error to
    that subcommand's OWN output schema (Phase 3: authorize's CLI errors must conform to
    SIDE_EFFECT_SCHEMA, not COMPLETION_SCHEMA, or a caller parsing `authorize` output would see a
    completion-manifest-shaped error envelope missing mode/action/authorization_state/allowed)."""

    def error(self, message: str) -> None:
        code, msg = "CLI_USAGE_ERROR", f"invalid command-line invocation: {message}"
        if self.prog.rsplit(" ", 1)[-1] == "authorize":
            _emit_authorization(_authorize_bare_result(None, None, code, msg), 2)
        _emit(_bare_result(code, msg), 2)


def main() -> None:
    ap = _GateArgumentParser(
        description="manifest-driven 3-state completion gate (runtime-ready | evidence-complete | promotion-ready)",
        epilog="exit codes: 0=success(promotion-eligible) 1=policy-block 2=invalid-input",
    )
    sub = ap.add_subparsers(dest="cmd", required=True)
    v = sub.add_parser("verify", help="evaluate a work-manifest and print a completion-manifest JSON to stdout")
    v.add_argument("--manifest", required=True, help="path to a work-manifest JSON file")
    v.add_argument("--repo-root", help="override repo root detection (bypasses .git lookup -- for hermetic tests)")
    v.set_defaults(func=cmd_verify)
    a = sub.add_parser("authorize", help="decide whether ONE side-effect action is authorized for a work-manifest")
    a.add_argument("--manifest", required=True, help="path to a work-manifest JSON file")
    a.add_argument("--mode", required=True, choices=["experimental", "promotion"])
    a.add_argument("--action", required=True, choices=list(ALLOWED_ACTIONS))
    a.add_argument("--repo-root", help="override repo root detection (bypasses .git lookup -- for hermetic tests)")
    a.set_defaults(func=cmd_authorize)
    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
