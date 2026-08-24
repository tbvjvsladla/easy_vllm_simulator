#!/usr/bin/env python3
"""evidence_publisher.py -- Hybrid evidence publisher (plan_26072506 Phase 2).

Implements the 7-step hybrid-publisher contract from plan_26072506 §4.4:
    1. compute a task_class's required_evidence list
    2. scaffold + a persisted publication/evidence record at task start
    3. accumulate raw evidence as work progresses (append-only, atomic)
    4. accept Sonnet-authored narrative as EXPLICIT external input (never invented here)
    5. delegate schema/path/identity/link/PII/verdict verification to the sibling completion_gate.py
       (never a second, reimplemented copy of those validators)
    6. restore/recreate any missing-but-reproducible scaffold item on rerun (idempotent)
    7. never fabricate: absent raw/narrative/verdict/certificate stays an explicit placeholder or
       blocker, not plausible prose

Deterministic-vs-Sonnet boundary (see .claude/rules/docs.md for the published contract):
    - Everything this script writes on its own (scaffold headers, provenance metadata, filenames,
      the publication record, raw-evidence log entries) is DETERMINISTIC: a pure function of its
      CLI arguments, never of a wall clock or of its own guesses about content.
    - The only prose this script ever places into a document is content it was given via an
      EXPLICIT FILE ARGUMENT (`set-narrative --narrative-file`, `publish-benchmark
      --bench-report-src`/`--certificate-src`) -- there is no code path that synthesizes
      narrative, measured numbers, a verdict, or a certificate from its own inference.

Subcommands: init | append-raw | set-narrative | publish-benchmark | finalize | (--self-test)

stdlib only, no third-party dependencies -- reuses the constitution completion gate and
wiki-desk owner-local doc_naming.py rather than reimplementing their logic (path-safety resolvers, the
required_evidence task-class matrix, the flat-certificate parser, and the actual gate/verdict
check are all imported, never duplicated).
"""
from __future__ import annotations

import argparse
import errno
import fcntl
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

_SCRIPTS_DIR = Path(__file__).resolve().parent
_REPO_ROOT = Path(__file__).resolve().parents[3]
_DOC_NAMING_DIR = _REPO_ROOT / ".claude" / "skills" / "wiki-desk" / "scripts"
for _module_dir in (_SCRIPTS_DIR, _DOC_NAMING_DIR):
    if str(_module_dir) not in sys.path:
        sys.path.insert(0, str(_module_dir))

import completion_gate as gate  # noqa: E402 -- SSOT reuse, not a duplicated matrix
import doc_naming  # noqa: E402

SCHEMA_VERSION = 1

# Re-exported, not reimplemented (TestRequiredEvidenceParity.test_same_function_object_as_completion_gate
# asserts this IS the same function object as completion_gate.required_evidence_for).
required_evidence_for = gate.required_evidence_for

# Evidence kinds this script can scaffold on its own (a fixed, deterministic path/content it owns).
# bench_report/certificate come only from an actual benchmark run (see `publish-benchmark`);
# verification/commit are pass-through pointers to evidence produced elsewhere (see `record-path`
# in a later phase of this same script). Scaffolding either would mean fabricating content this
# script has no basis for.
SCAFFOLDABLE_KINDS = ("plan", "devlog", "testlog", "simlog", "report")
DATED_KINDS = ("plan", "devlog", "testlog")
ALL_EVIDENCE_KINDS = {
    "plan", "devlog", "testlog", "simlog", "report", "verification", "commit",
    "bench_report", "certificate",
}

# capacity_rejection's required_evidence_for() only ever returns ("plan",) -- the devlog/testlog
# OR-group (completion_gate.py's own special case, not part of the reused matrix) is a convenience
# scaffold this script adds on top so a rerun trivially satisfies the gate either way. This is
# intentionally NOT folded into required_evidence_for()'s return value: doing so would make this
# module's "required_evidence" diverge from the imported SSOT it is required to match exactly.
CAPACITY_REJECTION_OR_GROUP = ("devlog", "testlog")

_NARRATIVE_BEGIN = "<!-- NARRATIVE:BEGIN -->"
_NARRATIVE_END = "<!-- NARRATIVE:END -->"
_PLACEHOLDER = "_PLACEHOLDER -- Sonnet narrative pending. Do not treat as final evidence._"


def _bare_error(code: str, message: str) -> dict:
    return {"schema_version": SCHEMA_VERSION, "ok": False, "reason_codes": [code], "messages": {code: message}}


def _emit(obj: dict, exit_code: int) -> None:
    print(json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2))
    raise SystemExit(exit_code)


def _load_json(path_str: str, label: str) -> dict:
    try:
        with open(path_str, "r", encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        _emit(_bare_error(f"{label.upper()}_FILE_NOT_FOUND", f"{label} file not found: {path_str}"), 2)
    except json.JSONDecodeError as e:
        _emit(_bare_error(f"{label.upper()}_INVALID_JSON", f"{label} file is not valid JSON: {e}"), 2)
    except UnicodeDecodeError as e:
        _emit(_bare_error(f"{label.upper()}_INVALID_UTF8", f"{label} file is not valid UTF-8: {e}"), 2)
    except OSError as e:
        _emit(_bare_error(f"{label.upper()}_FILE_UNREADABLE",
                          f"{label} file could not be read: {e}"), 2)


# =============================================================================
# Publication record I/O (the persisted "evidence manifest" -- plan_26072506 step 2)
# =============================================================================

def _record_path(repo_root: Path, topic: str) -> Path:
    return repo_root / "docs" / "_evidence" / ("%s.json" % topic)


# Nested publication-record fields every subcommand accesses via `.get()`/`[...]` assuming a dict
# (or, where noted, a dict-or-null) -- P2-A03: a syntactically-valid-JSON persisted record whose
# top level or one of these fields is the wrong shape (e.g. a bare `[]`, `null`, a scalar, or
# `{"scaffolded": [...]}`) must never reach one of those calls raw and surface as an uncaught
# AttributeError/TypeError traceback. Checked once, at the single load choke point every
# subcommand goes through (_load_record), rather than re-guarded ad hoc at each call site.
_RECORD_OBJECT_OR_NULL_FIELDS = ("capacity_rejection",)

# P2-FINAL-02: cmd_finalize indexes record["task_class"]/record["identity"] UNCONDITIONALLY (no
# `.get()`) -- a persisted record missing either key (e.g. a bare `{}`, or an object some other
# field of which was tampered away) reached that raw bracket access and surfaced an uncaught
# KeyError instead of a stable JSON exit-2 reject. Both are fields every real `init`-produced
# record always carries (init itself now also refuses to persist a non-object identity -- see
# INIT_IDENTITY_NOT_AN_OBJECT in cmd_init), so requiring them at the SAME single load choke point
# closes the gap without weakening any legitimate record.
_RECORD_REQUIRED_STRING_FIELDS = ("task_class",)
_RECORD_REQUIRED_OBJECT_FIELDS = (
    "identity", "conditions", "benchmark", "scaffolded", "narrative_status", "raw_log_paths",
)


def _validate_record_shape(record) -> tuple[str, str] | None:
    """Returns None if `record` has the shape every subcommand's `.get()`/`[...]` access on it
    assumes -- otherwise a stable (code, message) pair for the caller to `_emit` as one JSON
    reject at exit 2 (never a raw traceback)."""
    if not isinstance(record, dict):
        return ("PUBLICATION_RECORD_NOT_AN_OBJECT",
                f"publication record must be a JSON object, got {type(record).__name__}")
    allowed_fields = {
        "schema_version", "publication_id", "task_class", "generated_utc", "identity", "conditions",
        "benchmark", "required_evidence", "or_group_scaffolded", "capacity_rejection_required",
        "scaffolded", "narrative_status", "raw_log_paths", "capacity_rejection",
    }
    unknown_fields = set(record) - allowed_fields
    missing_fields = allowed_fields - set(record)
    if unknown_fields:
        return ("PUBLICATION_RECORD_UNKNOWN_FIELDS",
                f"publication record contains unknown fields {sorted(unknown_fields)}")
    if missing_fields:
        return ("PUBLICATION_RECORD_MISSING_FIELDS",
                f"publication record is missing producer fields {sorted(missing_fields)}")
    if not isinstance(record.get("publication_id"), str) or not record["publication_id"]:
        return ("PUBLICATION_RECORD_PUBLICATION_ID_INVALID",
                "publication_id must be a non-empty string")
    if (not isinstance(record.get("generated_utc"), str) or
            re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", record["generated_utc"]) is None):
        return ("PUBLICATION_RECORD_GENERATED_UTC_INVALID",
                "generated_utc must be a UTC second timestamp")
    for field in ("schema_version", "task_class", "identity", "conditions", "benchmark", "capacity_rejection"):
        if field == "capacity_rejection" and record[field] is None:
            continue
        errors = gate.validate_against_schema(
            record[field], gate.WORK_SCHEMA["properties"][field], gate.WORK_SCHEMA, field)
        if errors:
            return ("PUBLICATION_RECORD_SCHEMA_INVALID", errors[0][0])
    for field in ("required_evidence", "or_group_scaffolded"):
        if (not isinstance(record[field], list) or
                any(not isinstance(value, str) or value not in ALL_EVIDENCE_KINDS for value in record[field])):
            return (f"PUBLICATION_RECORD_FIELD_INVALID:{field}",
                    f"{field} must be a list of canonical evidence-kind strings")
    if not isinstance(record["capacity_rejection_required"], bool):
        return ("PUBLICATION_RECORD_CAPACITY_FLAG_INVALID",
                "capacity_rejection_required must be boolean")
    expected_required = required_evidence_for(
        record["task_class"], record["conditions"], record["benchmark"].get("verdict"))
    expected_or_group = (list(CAPACITY_REJECTION_OR_GROUP)
                         if record["task_class"] == "capacity_rejection" else [])
    expected_capacity_required = record["task_class"] == "capacity_rejection"
    if record["required_evidence"] != expected_required:
        return ("PUBLICATION_RECORD_REQUIRED_EVIDENCE_MISMATCH",
                "required_evidence does not match task_class/conditions/benchmark verdict")
    if record["or_group_scaffolded"] != expected_or_group:
        return ("PUBLICATION_RECORD_OR_GROUP_MISMATCH",
                "or_group_scaffolded does not match the task-class producer contract")
    if record["capacity_rejection_required"] != expected_capacity_required:
        return ("PUBLICATION_RECORD_CAPACITY_FLAG_MISMATCH",
                "capacity_rejection_required does not match task_class")
    for field in _RECORD_REQUIRED_STRING_FIELDS:
        if field not in record:
            return (f"PUBLICATION_RECORD_MISSING_FIELD:{field}",
                    f"publication record is missing required field {field!r}")
        if not isinstance(record[field], str):
            return (f"PUBLICATION_RECORD_FIELD_WRONG_TYPE:{field}",
                    f"publication record field {field!r} must be a string, "
                    f"got {type(record[field]).__name__}")
    for field in _RECORD_REQUIRED_OBJECT_FIELDS:
        if field not in record:
            return (f"PUBLICATION_RECORD_MISSING_FIELD:{field}",
                    f"publication record is missing required field {field!r}")
        if not isinstance(record[field], dict):
            return (f"PUBLICATION_RECORD_FIELD_NOT_AN_OBJECT:{field}",
                    f"publication record field {field!r} must be a JSON object, "
                    f"got {type(record[field]).__name__}")
    for field in _RECORD_OBJECT_OR_NULL_FIELDS:
        if field in record and record[field] is not None and not isinstance(record[field], dict):
            return (f"PUBLICATION_RECORD_FIELD_NOT_AN_OBJECT:{field}",
                    f"publication record field {field!r} must be a JSON object (or null), "
                    f"got {type(record[field]).__name__}")
    # "Do not accept unknown dangerous path shapes": scaffolded.* values are re-trusted as
    # write/read path targets elsewhere (_validate_prior_path calls path_str.startswith(...)) -- a
    # non-string, non-null value there (e.g. an int, list) would raise an AttributeError the first
    # time any command re-validates it as a path, rather than surfacing a stable reject here.
    scaffolded = record.get("scaffolded")
    if isinstance(scaffolded, dict):
        allowed_scaffolded = set(ALL_EVIDENCE_KINDS)
        actual_keys = set(scaffolded)
        if actual_keys != allowed_scaffolded:
            unknown = actual_keys - allowed_scaffolded
            missing = allowed_scaffolded - actual_keys
            return ("PUBLICATION_RECORD_SCAFFOLDED_KEYS_INVALID",
                    f"scaffolded must contain the complete producer key set; "
                    f"unknown={sorted(unknown)} missing={sorted(missing)}")
        for kind, value in scaffolded.items():
            if value is not None and (not isinstance(value, str) or not value):
                return (f"PUBLICATION_RECORD_SCAFFOLDED_VALUE_NOT_A_STRING:{kind}",
                        f"publication record field 'scaffolded.{kind}' must be a non-empty string path or null, "
                        f"got {type(value).__name__}")
    narrative_status = record.get("narrative_status")
    if isinstance(narrative_status, dict):
        unknown = set(narrative_status) - set(SCAFFOLDABLE_KINDS)
        if unknown:
            return ("PUBLICATION_RECORD_NARRATIVE_STATUS_UNKNOWN_KEYS",
                    f"narrative_status contains unknown evidence kinds {sorted(unknown)}")
        expected_entry_keys = {"status", "author", "generated_utc", "source"}
        for kind, value in narrative_status.items():
            if not isinstance(value, dict):
                return (f"PUBLICATION_RECORD_NARRATIVE_STATUS_VALUE_NOT_AN_OBJECT:{kind}",
                        f"publication record field 'narrative_status.{kind}' must be a JSON "
                        f"object, got {type(value).__name__}")
            if set(value) != expected_entry_keys or value.get("status") != "authored":
                return (f"PUBLICATION_RECORD_NARRATIVE_STATUS_SHAPE_INVALID:{kind}",
                        f"narrative_status.{kind} must be the complete producer-emitted authored entry")
            if any(not isinstance(value[key], str) or not value[key]
                   for key in ("author", "generated_utc", "source")):
                return (f"PUBLICATION_RECORD_NARRATIVE_STATUS_FIELD_INVALID:{kind}",
                        f"narrative_status.{kind} provenance fields must be non-empty strings")
            if re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value["generated_utc"]) is None:
                return (f"PUBLICATION_RECORD_NARRATIVE_STATUS_TIME_INVALID:{kind}",
                        f"narrative_status.{kind}.generated_utc must be a UTC second timestamp")
    raw_log_paths = record.get("raw_log_paths")
    if isinstance(raw_log_paths, dict):
        unknown = set(raw_log_paths) - set(APPEND_RAW_KINDS)
        if unknown:
            return ("PUBLICATION_RECORD_RAW_LOG_PATHS_UNKNOWN_KEYS",
                    f"raw_log_paths contains unknown evidence kinds {sorted(unknown)}")
        for key, value in record["raw_log_paths"].items():
            if not isinstance(value, str) or not value:
                return (f"PUBLICATION_RECORD_RAW_LOG_PATH_INVALID:{key}",
                        f"raw_log_paths.{key} must be a nonempty string")
            expected_path = f"docs/_evidence/{record['publication_id']}.{key}.raw.jsonl"
            if value != expected_path:
                return (f"PUBLICATION_RECORD_RAW_LOG_PATH_MISMATCH:{key}",
                        f"raw_log_paths.{key} must equal the producer-derived sidecar path")
    return None


def _load_record(repo_root: Path, topic: str) -> dict | None:
    """P2-FINAL-01: docs/_evidence/ (and the record file itself) is read through the SAME
    symlink-rejecting, dir_fd-relative machinery every publisher-controlled WRITE destination goes
    through (_open_evidence_dir_fd/_safe_stat_in_dir/O_NOFOLLOW) -- a symlinked docs/ or
    docs/_evidence/ path component, or a record basename that is itself a symlink, is rejected by
    construction rather than merely resolved (Path.is_file()/open()) and trusted."""
    error_prefix = "PUBLICATION_RECORD"
    basename = "%s.json" % topic
    dir_fd = _open_evidence_dir_fd(repo_root, error_prefix)
    try:
        kind = _safe_stat_in_dir(dir_fd, basename)
        if kind == "missing":
            return None
        if kind == "symlink":
            _emit(_bare_error("PUBLICATION_RECORD_TARGET_IS_SYMLINK",
                              f"publication record for topic {topic!r} is a symlink -- refusing "
                              f"to read through it"), 2)
        if kind != "file":
            _emit(_bare_error("PUBLICATION_RECORD_TARGET_WRONG_TYPE",
                              f"publication record for topic {topic!r} is not a regular file "
                              f"(kind={kind!r})"), 2)
        try:
            fd = os.open(basename, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=dir_fd)
        except OSError as e:
            if e.errno == errno.ELOOP:
                _emit(_bare_error("PUBLICATION_RECORD_TARGET_IS_SYMLINK",
                                  f"publication record for topic {topic!r} is a symlink -- "
                                  f"refusing to read through it"), 2)
            _emit(_bare_error("PUBLICATION_RECORD_IO_ERROR",
                              f"could not open publication record for topic {topic!r}: {e}"), 2)
        try:
            with os.fdopen(fd, "r", encoding="utf-8") as f:
                text = f.read()
        except UnicodeDecodeError as e:
            _emit(_bare_error("PUBLICATION_RECORD_INVALID_UTF8",
                              f"publication record for topic {topic!r} is not valid UTF-8: {e}"), 2)
    finally:
        os.close(dir_fd)
    try:
        record = json.loads(text)
    except json.JSONDecodeError as e:
        _emit(_bare_error("PUBLICATION_RECORD_INVALID_JSON",
                          f"publication record for topic {topic!r} is not valid JSON: {e}"), 2)
    shape_error = _validate_record_shape(record)
    if shape_error:
        code, message = shape_error
        _emit(_bare_error(code, f"publication record for topic {topic!r}: {message}"), 2)
    if record["publication_id"] != topic:
        _emit(_bare_error(
            "PUBLICATION_RECORD_TOPIC_MISMATCH",
            f"publication record selected as topic {topic!r} declares publication_id "
            f"{record['publication_id']!r}"), 2)
    return record


def _validate_topic_or_die(topic: str) -> None:
    """CLI-boundary guard for `--topic`: reused doc_naming.validate_topic() (single SSOT, not a
    second, divergent check) rejects path separators/whitespace/empty/'.'/'..' BEFORE topic is
    ever used to build ANY path (the publication record path, the raw-evidence sidecar path, or
    any scaffold path) -- this is what stops `--topic ../escaped` from ever reaching
    _record_path()."""
    try:
        doc_naming.validate_topic(topic)
    except ValueError as e:
        _emit(_bare_error("INVALID_TOPIC", str(e)), 2)


def _validate_required_utc_timestamp(value: str, arg_name: str) -> None:
    """CLI-boundary guard for a REQUIRED --generated-utc/--recorded-utc argument. doc_naming.
    kst_tokens() is intentionally fail-soft for ITS callers (unparseable -> 'NA' sentinel, an
    N/A-fail-soft convention for optional consumers like bench-certificate meta) -- but a
    REQUIRED CLI timestamp argument that doesn't parse into a real calendar date (including a
    string that merely LOOKS like 'YYYY-MM-DDTHH:MM:SSZ' digit-shape but names an impossible date,
    e.g. '2026-99-99T99:99:99Z') is a user-input error here, not a legitimate 'not given' case."""
    yymmddhh, _mm, _ss = doc_naming.kst_tokens(value)
    if yymmddhh == "NA":
        code = "INVALID_%s" % arg_name.upper().replace("-", "_")
        _emit(_bare_error(code, f"--{arg_name} {value!r} is not a valid UTC 'YYYY-MM-DDTHH:MM:SSZ' timestamp"), 2)


# =============================================================================
# Hardened, symlink-rejecting, dir_fd/O_NOFOLLOW write-path safety (P2-02: output
# containment/symlink safety -- every publisher-controlled WRITE destination is verified this way;
# persisted record paths are re-validated through the SAME machinery, never trusted as-is).
# =============================================================================

def _safe_mkdir_chain(repo_root_fd: int, parts: list[str], error_prefix: str) -> int:
    """dir_fd-relative walk from repo_root_fd through `parts`, O_NOFOLLOW at every component,
    CREATING intermediate directories as needed. A pre-existing symlink at any component, or one
    planted between our mkdir and open (TOCTOU), is rejected by construction (O_NOFOLLOW makes
    that open() fail with ELOOP) -- this does not merely `os.path.normpath` and hope. Returns an
    open dir_fd for the final directory; caller must os.close() it. Fails closed (stable JSON,
    exit 2) on any symlink/unexpected-type component -- never silently redirects."""
    cur_fd = os.dup(repo_root_fd)
    for part in parts:
        try:
            os.mkdir(part, dir_fd=cur_fd)
        except FileExistsError:
            pass
        except OSError as e:
            os.close(cur_fd)
            _emit(_bare_error(f"{error_prefix}_DIR_CREATE_FAILED",
                              f"could not create directory component {part!r}: {e}"), 2)
        try:
            next_fd = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=cur_fd)
        except OSError as e:
            os.close(cur_fd)
            if e.errno == errno.ELOOP:
                _emit(_bare_error(f"{error_prefix}_SYMLINK",
                                  f"path component {part!r} is a symlink -- refusing to write through it"), 2)
            _emit(_bare_error(f"{error_prefix}_DIR_OPEN_FAILED", f"path component {part!r} is unsafe or unreadable: {e}"), 2)
        os.close(cur_fd)
        cur_fd = next_fd
    return cur_fd


def _safe_stat_in_dir(dir_fd: int, basename: str) -> str:
    """Returns 'file'|'dir'|'symlink'|'missing'|'other' for `basename` inside dir_fd, via lstat
    (never follows a symlink to stat its target)."""
    try:
        st = os.lstat(basename, dir_fd=dir_fd)
    except FileNotFoundError:
        return "missing"
    except OSError:
        return "other"
    if stat.S_ISLNK(st.st_mode):
        return "symlink"
    if stat.S_ISREG(st.st_mode):
        return "file"
    if stat.S_ISDIR(st.st_mode):
        return "dir"
    return "other"


def _safe_listdir_names(dir_fd: int) -> set:
    try:
        return set(os.listdir(dir_fd))
    except OSError:
        return set()


def _safe_replace_file(dir_fd: int, basename: str, content_bytes: bytes, error_prefix: str) -> None:
    """Atomic create/replace of `basename` inside the directory referenced by dir_fd: writes to a
    dir_fd-relative temp file (O_NOFOLLOW, so it can never itself be a symlink target), fsyncs,
    then os.replace(..., src_dir_fd=dir_fd, dst_dir_fd=dir_fd) -- both endpoints resolved against
    the SAME already-symlink-verified descriptor, so nothing between our mkdir/open walk and this
    replace can swap the destination out from under us."""
    if os.sep in basename or basename in (".", ".."):
        _emit(_bare_error(f"{error_prefix}_BASENAME_INVALID", f"unsafe basename: {basename!r}"), 2)
    existing_kind = _safe_stat_in_dir(dir_fd, basename)
    if existing_kind == "symlink":
        _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                          f"{basename!r} is a symlink -- refusing to write through it"), 2)
    if existing_kind == "dir":
        _emit(_bare_error(f"{error_prefix}_TARGET_WRONG_TYPE", f"{basename!r} is a directory, expected a file"), 2)
    tmp_name = basename + ".tmp-%d" % os.getpid()
    try:
        fd = os.open(tmp_name, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o644, dir_fd=dir_fd)
    except OSError as e:
        _emit(_bare_error(f"{error_prefix}_TMP_CREATE_FAILED", f"could not create temp file: {e}"), 2)
    try:
        os.write(fd, content_bytes)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(tmp_name, basename, src_dir_fd=dir_fd, dst_dir_fd=dir_fd)


def _open_evidence_dir_fd(repo_root: Path, error_prefix: str) -> int:
    """P2-FINAL-01: opens (creating as needed) docs/_evidence/ via the SAME symlink-rejecting,
    dir_fd-relative walk used for every other publisher-controlled write destination
    (_safe_mkdir_chain) -- a symlinked docs/ or docs/_evidence/ path component is rejected by
    construction (O_NOFOLLOW => ELOOP) rather than merely resolved (Path/open) and trusted. Every
    read or write of the publication record, a JSONL raw-evidence sidecar, or the finalize
    work-manifest goes through this ONE choke point. Caller must os.close() the returned fd."""
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        return _safe_mkdir_chain(repo_root_fd, ["docs", "_evidence"], error_prefix)
    finally:
        os.close(repo_root_fd)


def _write_evidence_dir_file(repo_root: Path, basename: str, content_bytes: bytes, error_prefix: str) -> None:
    """P2-FINAL-01: the one write path every docs/_evidence/<basename> destination in this module
    goes through (the publication record, and finalize's work-manifest.json) -- reuses/generalizes
    the SAME hardened dir_fd/O_NOFOLLOW machinery already used for scaffold writes
    (_open_evidence_dir_fd + _safe_replace_file) rather than an ordinary Path.mkdir()/open() that
    would silently follow a symlinked docs/ or docs/_evidence/ path component."""
    dir_fd = _open_evidence_dir_fd(repo_root, error_prefix)
    try:
        _safe_replace_file(dir_fd, basename, content_bytes, error_prefix)
    finally:
        os.close(dir_fd)


def _validate_prior_path(repo_root: Path, prior_rel_path: str, expected_prefix: tuple, error_prefix: str) -> list[str]:
    """Distrust persisted record paths: re-validates a scaffolded-path STRING loaded from the
    publication record BEFORE it is used as a write target at all -- a tampered record (e.g.
    scaffolded.plan rewritten to 'outside.md' or '../../../etc/x') is never trusted just because
    it came from a file we ourselves wrote earlier. Returns the path's repo-relative component
    list on success; fails closed (stable JSON, exit 2) otherwise. This is defense BEFORE any
    filesystem access -- _safe_mkdir_chain/_safe_stat_in_dir independently re-verify no symlink
    exists at actual access time, so this is one layer, not the only one."""
    if gate._is_absolute_path_string(prior_rel_path):
        _emit(_bare_error(f"{error_prefix}_RECORD_PATH_ABSOLUTE",
                          f"publication record's path {prior_rel_path!r} is absolute -- refusing to trust it"), 2)
    lex_status, parts = gate._lexical_components(repo_root, repo_root, prior_rel_path)
    if lex_status != "ok":
        _emit(_bare_error(f"{error_prefix}_RECORD_PATH_ESCAPES_REPO",
                          f"publication record's path {prior_rel_path!r} escapes the repo root -- refusing to trust it"), 2)
    if tuple(parts[:len(expected_prefix)]) != tuple(expected_prefix):
        _emit(_bare_error(f"{error_prefix}_RECORD_PATH_UNEXPECTED_LOCATION",
                          f"publication record's path {prior_rel_path!r} is not under "
                          f"{'/'.join(expected_prefix)}/ -- refusing to trust a tampered record"), 2)
    if len(parts) <= len(expected_prefix):
        _emit(_bare_error(f"{error_prefix}_RECORD_PATH_UNEXPECTED_LOCATION",
                          f"publication record's path {prior_rel_path!r} has no basename under "
                          f"{'/'.join(expected_prefix)}/"), 2)
    return parts


def _save_record(repo_root: Path, topic: str, record: dict) -> Path:
    """P2-FINAL-01: writes through the SAME hardened dir_fd/O_NOFOLLOW machinery every other
    publisher-controlled write destination uses -- a symlinked docs/ or docs/_evidence/ path
    component, or a record basename that is itself a symlink, is rejected by construction rather
    than merely resolved (Path.mkdir()/open()) and trusted."""
    content = json.dumps(record, ensure_ascii=False, sort_keys=True, indent=2).encode("utf-8")
    _write_evidence_dir_file(repo_root, "%s.json" % topic, content, "PUBLICATION_RECORD")
    return _record_path(repo_root, topic)


def _repo_relative(repo_root: Path, path: Path) -> str:
    return str(path.relative_to(repo_root)).replace(os.sep, "/")


def _open_repo_root_fd(repo_root: Path) -> int:
    try:
        return os.open(str(repo_root), os.O_RDONLY | os.O_DIRECTORY)
    except OSError as e:
        _emit(_bare_error("REPO_ROOT_UNREADABLE", f"could not open repo root {repo_root}: {e}"), 2)


def _resolve_repo_root(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit).resolve()
    found = gate._find_repo_root(Path(__file__))
    if found is None:
        _emit(_bare_error("REPO_ROOT_NOT_FOUND",
                          "cannot locate repo root (.git not found) from script location; pass --repo-root explicitly"), 2)
    return found


# =============================================================================
# Scaffold content (deterministic templates -- no invented narrative)
# =============================================================================

def _scaffold_markdown(doc_kind: str, topic: str, task_class: str, generated_utc) -> str:
    return (
        "# %s -- %s\n\n"
        "<!-- evidence_publisher scaffold -- provenance -->\n"
        "- publication_id: %s\n"
        "- task_class: %s\n"
        "- generated_utc: %s\n"
        "- scaffold_created_by: evidence_publisher.py init\n\n"
        "## Narrative\n\n"
        "%s\n"
        "%s\n"
        "%s\n"
    ) % (doc_kind, topic, topic, task_class, generated_utc, _NARRATIVE_BEGIN, _PLACEHOLDER, _NARRATIVE_END)


def _ensure_dated_scaffold(repo_root_fd: int, repo_root: Path, doc_kind: str, topic: str, task_class: str,
                            generated_utc, prior_rel_path: str | None) -> tuple[str, bool]:
    """Returns (repo_relative_path, created_now). Reuses the PRIOR recorded path when present
    (idempotency) -- restores the file if it went missing, leaves it untouched if present. The
    persisted path is NEVER trusted as-is (P2-02): it is re-validated (_validate_prior_path) and
    then accessed only through the symlink-rejecting dir_fd walk."""
    error_prefix = "SCAFFOLD_%s" % doc_kind.upper()
    if prior_rel_path:
        parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", doc_kind), error_prefix)
        dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
        try:
            basename = parts[-1]
            kind_of = _safe_stat_in_dir(dir_fd, basename)
            if kind_of == "file":
                return prior_rel_path, False
            if kind_of == "symlink":
                _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                                  f"{prior_rel_path!r} is a symlink -- refusing to write through it"), 2)
            if kind_of == "dir":
                _emit(_bare_error(f"{error_prefix}_TARGET_WRONG_TYPE",
                                  f"{prior_rel_path!r} is a directory, expected a file"), 2)
            content = _scaffold_markdown(doc_kind, topic, task_class, generated_utc).encode("utf-8")
            _safe_replace_file(dir_fd, basename, content, error_prefix)
        finally:
            os.close(dir_fd)
        return prior_rel_path, True
    dir_fd = _safe_mkdir_chain(repo_root_fd, ["docs", doc_kind], error_prefix)
    try:
        existing_basenames = _safe_listdir_names(dir_fd)
        try:
            basename = doc_naming.dated_doc_basename(doc_kind, generated_utc, topic, existing_basenames=existing_basenames)
        except doc_naming.NamingCollisionExhausted as e:
            _emit(_bare_error(f"{error_prefix}_COLLISION_EXHAUSTED", str(e)), 2)
        content = _scaffold_markdown(doc_kind, topic, task_class, generated_utc).encode("utf-8")
        _safe_replace_file(dir_fd, basename, content, error_prefix)
    finally:
        os.close(dir_fd)
    return "docs/%s/%s" % (doc_kind, basename), True


def _ensure_simlog_scaffold(repo_root_fd: int, repo_root: Path, topic: str, generated_utc,
                             prior_rel_path: str | None) -> tuple[str, bool]:
    error_prefix = "SCAFFOLD_SIMLOG"
    if prior_rel_path:
        parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", "simlog"), error_prefix)
        dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
        try:
            basename = parts[-1]
            kind_of = _safe_stat_in_dir(dir_fd, basename)
            if kind_of == "dir":
                return prior_rel_path, False
            if kind_of == "symlink":
                _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                                  f"{prior_rel_path!r} is a symlink -- refusing to use it as the simlog run directory"), 2)
            if kind_of == "file":
                _emit(_bare_error(f"{error_prefix}_TARGET_WRONG_TYPE",
                                  f"{prior_rel_path!r} is a file, expected a directory"), 2)
            try:
                os.mkdir(basename, dir_fd=dir_fd)
            except FileExistsError:
                pass
            except OSError as e:
                _emit(_bare_error(f"{error_prefix}_DIR_CREATE_FAILED", f"could not restore simlog run directory: {e}"), 2)
        finally:
            os.close(dir_fd)
        return prior_rel_path, True
    dir_fd = _safe_mkdir_chain(repo_root_fd, ["docs", "simlog"], error_prefix)
    try:
        existing_dirnames = _safe_listdir_names(dir_fd)
        try:
            dirname = doc_naming.simlog_dirname(generated_utc, topic, existing_dirnames=existing_dirnames)
        except doc_naming.NamingCollisionExhausted as e:
            _emit(_bare_error(f"{error_prefix}_COLLISION_EXHAUSTED", str(e)), 2)
        try:
            os.mkdir(dirname, dir_fd=dir_fd)
        except FileExistsError:
            pass
        except OSError as e:
            _emit(_bare_error(f"{error_prefix}_DIR_CREATE_FAILED", f"could not create simlog run directory: {e}"), 2)
    finally:
        os.close(dir_fd)
    return "docs/simlog/%s" % dirname, True


def _ensure_report_scaffold(repo_root_fd: int, repo_root: Path, topic: str, task_class: str, generated_utc,
                             report_slug: str, prior_rel_path: str | None) -> tuple[str, bool]:
    error_prefix = "SCAFFOLD_REPORT"
    if prior_rel_path:
        parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", "report"), error_prefix)
        dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
        try:
            basename = parts[-1]
            kind_of = _safe_stat_in_dir(dir_fd, basename)
            if kind_of == "file":
                return prior_rel_path, False
            if kind_of == "symlink":
                _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                                  f"{prior_rel_path!r} is a symlink -- refusing to write through it"), 2)
            if kind_of == "dir":
                _emit(_bare_error(f"{error_prefix}_TARGET_WRONG_TYPE",
                                  f"{prior_rel_path!r} is a directory, expected a file"), 2)
            content = _scaffold_markdown("report", topic, task_class, generated_utc).encode("utf-8")
            _safe_replace_file(dir_fd, basename, content, error_prefix)
        finally:
            os.close(dir_fd)
        return prior_rel_path, True
    try:
        basename = doc_naming.report_basename(report_slug)
    except ValueError as e:
        _emit(_bare_error("INIT_REPORT_SLUG_INVALID", str(e)), 2)
    dir_fd = _safe_mkdir_chain(repo_root_fd, ["docs", "report"], error_prefix)
    try:
        content = _scaffold_markdown("report", topic, task_class, generated_utc).encode("utf-8")
        _safe_replace_file(dir_fd, basename, content, error_prefix)
    finally:
        os.close(dir_fd)
    return "docs/report/%s" % basename, True


# =============================================================================
# Shared raw-evidence source-path safety resolution (reuses completion_gate.py's hardened
# O_NOFOLLOW/dir_fd path resolver rather than reimplementing a second copy).
# =============================================================================

APPEND_RAW_KINDS = ("plan", "devlog", "testlog", "simlog", "report", "verification", "commit")
_JSONL_SIDECAR_KINDS = ("plan", "devlog", "testlog", "report", "verification", "commit")
_POINTER_ONLY_KINDS = ("verification", "commit")  # no init-time scaffold; the raw src IS the evidence path


def _resolve_src(repo_root: Path, src: str, error_prefix: str) -> tuple[bytes, str]:
    """Validates `src` as a safe, repo-relative, existing non-empty regular UTF-8 text file.
    Returns (content_bytes, sha256). On any violation, emits ONE stable JSON reject and exits 2 --
    every anticipated failure mode is handled explicitly here, so no Python traceback is ever the
    visible result of a rejected input."""
    if gate._is_absolute_path_string(src):
        _emit(_bare_error(f"{error_prefix}_ABSOLUTE_SRC", f"src path must be repo-relative, not absolute: {src!r}"), 2)
    try:
        repo_root_fd = os.open(str(repo_root), os.O_RDONLY | os.O_DIRECTORY)
    except OSError as e:
        _emit(_bare_error(f"{error_prefix}_REPO_ROOT_UNREADABLE", f"could not open repo root {repo_root}: {e}"), 2)
    try:
        r = gate.resolve_and_stat_evidence(repo_root_fd, repo_root, repo_root, src, expect_dir=False, capture_content=True)
    finally:
        os.close(repo_root_fd)
    if r["status"] == "repo_escape":
        _emit(_bare_error(f"{error_prefix}_SRC_ESCAPES_REPO", f"src path escapes the repo root via '..' traversal: {src!r}"), 2)
    if r["status"] == "symlink":
        _emit(_bare_error(f"{error_prefix}_SRC_SYMLINK", f"src path contains a symlink at some path component: {src!r}"), 2)
    if r["status"] in ("not_found", "not_a_directory"):
        _emit(_bare_error(f"{error_prefix}_SRC_NOT_FOUND", f"src path does not resolve to an existing file: {src!r}"), 2)
    if r["status"] == "wrong_type":
        _emit(_bare_error(f"{error_prefix}_SRC_WRONG_TYPE", f"src path is not a non-empty regular file: {src!r}"), 2)
    if r["status"] == "os_error":
        _emit(_bare_error(f"{error_prefix}_SRC_UNREADABLE", f"src path could not be read: {src!r}"), 2)
    content_bytes = r["content_bytes"] or b""
    try:
        content_bytes.decode("utf-8")
    except UnicodeDecodeError as e:
        _emit(_bare_error(f"{error_prefix}_SRC_INVALID_UTF8", f"src file is not valid UTF-8: {e}"), 2)
    return content_bytes, r["sha256"]


def _require_record(repo_root: Path, topic: str) -> dict:
    record = _load_record(repo_root, topic)
    if record is None:
        _emit(_bare_error("PUBLICATION_NOT_INITIALIZED",
                          f"no publication record for topic {topic!r} -- run `init` first"), 2)
    return record


def _jsonl_sidecar_path(repo_root: Path, topic: str, kind: str) -> Path:
    return repo_root / "docs" / "_evidence" / ("%s.%s.raw.jsonl" % (topic, kind))


# =============================================================================
# `append-raw` subcommand (plan_26072506 hybrid-publisher step 3: accumulate raw evidence,
# append-only/atomic, repo-relative, fail-closed on anything unsafe).
# =============================================================================

def _validate_jsonl_ledger_bytes(content: bytes, error_prefix: str) -> int:
    """P2-FINAL-03: validates the FULL prior sidecar content under the same lock hold that is
    about to append to it -- every nonempty line must be a JSON object with an integer 'seq'
    field, seq values must be exactly contiguous 1..N in file order, and a nonempty ledger must be
    newline-terminated (no trailing partial/malformed bytes). An existing ledger's line count was
    previously trusted via a bare `\\n`-count with no shape/order check at all. Returns N (0 for an
    empty/fresh sidecar). Fails closed (stable JSON, exit 2) on ANY violation -- the caller must
    never append past an untrustworthy existing ledger."""
    if not content:
        return 0
    if not content.endswith(b"\n"):
        _emit(_bare_error(f"{error_prefix}_LEDGER_NOT_NEWLINE_TERMINATED",
                          "existing raw-evidence ledger does not end with a newline -- refusing "
                          "to trust or append to a possibly-truncated/malformed ledger"), 2)
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as e:
        _emit(_bare_error(f"{error_prefix}_LEDGER_INVALID_UTF8",
                          f"existing raw-evidence ledger is not valid UTF-8: {e}"), 2)
    lines = text.split("\n")[:-1]  # trailing "" from the newline terminator we just required
    for i, line in enumerate(lines):
        if not line:
            _emit(_bare_error(f"{error_prefix}_LEDGER_BLANK_LINE",
                              f"existing raw-evidence ledger has a blank line at position {i + 1}"), 2)
        try:
            entry = json.loads(line)
        except json.JSONDecodeError as e:
            _emit(_bare_error(f"{error_prefix}_LEDGER_MALFORMED_ENTRY",
                              f"existing raw-evidence ledger line {i + 1} is not valid JSON: {e}"), 2)
        if not isinstance(entry, dict):
            _emit(_bare_error(f"{error_prefix}_LEDGER_ENTRY_NOT_AN_OBJECT",
                              f"existing raw-evidence ledger line {i + 1} is not a JSON object"), 2)
        seq = entry.get("seq")
        if not isinstance(seq, int) or isinstance(seq, bool):
            _emit(_bare_error(f"{error_prefix}_LEDGER_SEQ_WRONG_TYPE",
                              f"existing raw-evidence ledger line {i + 1} has a non-integer 'seq'"), 2)
        if seq != i + 1:
            _emit(_bare_error(f"{error_prefix}_LEDGER_SEQ_NOT_CONTIGUOUS",
                              f"existing raw-evidence ledger line {i + 1} has seq={seq}, expected "
                              f"{i + 1} -- ledger must be duplicate-free, gap-free, and in order"), 2)
    return len(lines)


def _append_jsonl_locked(dir_fd: int, basename: str, entry_without_seq: dict, error_prefix: str) -> int:
    """Read-validate-count -> compute-seq -> append-line as ONE interprocess critical section,
    guarded by an exclusive Linux `flock` on the sidecar file itself (advisory, blocking, tied to
    the open file description -- correctly serializes concurrent OS processes, not just threads
    within one process). P2-FINAL-01: the sidecar is opened via a dir_fd-relative, O_NOFOLLOW open
    of `basename` against the ALREADY symlink-verified docs/_evidence/ directory descriptor -- a
    sidecar path that is itself a symlink is rejected by construction, never resolved-then-
    trusted. Under that SAME lock hold, P2-FINAL-03 validates the full prior ledger
    (_validate_jsonl_ledger_bytes) BEFORE any append happens -- an untrustworthy existing ledger
    is rejected (stable JSON, exit 2, no mutation) rather than silently trusted for its line count
    the way a bare `\\n`-count used to be. Returns the assigned seq (1-based)."""
    existing_kind = _safe_stat_in_dir(dir_fd, basename)
    if existing_kind == "symlink":
        _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                          f"{basename!r} is a symlink -- refusing to append through it"), 2)
    if existing_kind not in ("missing", "file"):
        _emit(_bare_error(f"{error_prefix}_TARGET_WRONG_TYPE",
                          f"{basename!r} is not a regular file (kind={existing_kind!r})"), 2)
    try:
        fd = os.open(basename, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o644, dir_fd=dir_fd)
    except OSError as e:
        if e.errno == errno.ELOOP:
            _emit(_bare_error(f"{error_prefix}_TARGET_IS_SYMLINK",
                              f"{basename!r} is a symlink -- refusing to append through it"), 2)
        _emit(_bare_error(f"{error_prefix}_IO_ERROR", f"could not open {basename!r}: {e}"), 2)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX)
        try:
            os.lseek(fd, 0, os.SEEK_SET)
            content = bytearray()
            while True:
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                content.extend(chunk)
            prior_count = _validate_jsonl_ledger_bytes(bytes(content), error_prefix)
            seq = prior_count + 1
            entry = dict(entry_without_seq, seq=seq)
            line = (json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n").encode("utf-8")
            os.lseek(fd, 0, os.SEEK_END)
            os.write(fd, line)
            os.fsync(fd)
            return seq
        finally:
            fcntl.flock(fd, fcntl.LOCK_UN)
    finally:
        os.close(fd)


def cmd_append_raw(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    _validate_required_utc_timestamp(args.recorded_utc, "recorded-utc")
    record = _require_record(repo_root, args.topic)

    if args.kind not in APPEND_RAW_KINDS:
        _emit(_bare_error("APPEND_RAW_UNSUPPORTED_KIND",
                          f"kind {args.kind!r} is not accepted by append-raw (bench_report/certificate "
                          f"come only from `publish-benchmark`, which validates them against the real "
                          f"benchmark verdict rather than accepting arbitrary raw text)"), 2)

    content_bytes, sha256 = _resolve_src(repo_root, args.src, "RAW_EVIDENCE")

    if args.kind == "simlog":
        simlog_rel = record.get("scaffolded", {}).get("simlog")
        if not simlog_rel:
            _emit(_bare_error("RAW_EVIDENCE_SIMLOG_NOT_SCAFFOLDED",
                              "simlog run directory not scaffolded yet for this topic -- run `init` "
                              "with a task_class/conditions that require simlog first"), 2)
        error_prefix = "RAW_EVIDENCE_SIMLOG"
        parts = _validate_prior_path(repo_root, simlog_rel, ("docs", "simlog"), error_prefix)
        repo_root_fd = _open_repo_root_fd(repo_root)
        try:
            dir_fd = _safe_mkdir_chain(repo_root_fd, parts, error_prefix)
        finally:
            os.close(repo_root_fd)
        try:
            dest_name = args.dest_name or Path(args.src).name
            if not dest_name or os.sep in dest_name or dest_name in (".", ".."):
                _emit(_bare_error("RAW_EVIDENCE_DEST_NAME_INVALID", f"invalid --dest-name: {dest_name!r}"), 2)
            existing_kind = _safe_stat_in_dir(dir_fd, dest_name)
            if existing_kind != "missing":
                _emit(_bare_error("RAW_EVIDENCE_DEST_COLLISION",
                                  f"a file named {dest_name!r} already exists in the simlog run directory -- "
                                  f"append-only: pass a distinct --dest-name rather than overwriting existing "
                                  f"raw evidence"), 2)
            # O_EXCL: atomically fails if another concurrent caller created the SAME dest_name
            # between our check and this open -- race-safe, not merely check-then-write.
            try:
                fd = os.open(dest_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644, dir_fd=dir_fd)
            except FileExistsError:
                _emit(_bare_error("RAW_EVIDENCE_DEST_COLLISION",
                                  f"a file named {dest_name!r} was just created concurrently -- "
                                  f"pass a distinct --dest-name"), 2)
            try:
                os.write(fd, content_bytes)
                os.fsync(fd)
            finally:
                os.close(fd)
        finally:
            os.close(dir_fd)
        recorded_rel = "%s/%s" % (simlog_rel, dest_name)
        _emit({
            "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
            "publication_id": args.topic, "kind": args.kind, "recorded_path": recorded_rel, "sha256": sha256,
        }, 0)

    sidecar_rel = _jsonl_sidecar_path(repo_root, args.topic, args.kind)
    sidecar_error_prefix = "RAW_EVIDENCE_SIDECAR"
    sidecar_dir_fd = _open_evidence_dir_fd(repo_root, sidecar_error_prefix)
    try:
        seq = _append_jsonl_locked(sidecar_dir_fd, sidecar_rel.name,
                                    {"src": args.src, "sha256": sha256, "recorded_utc": args.recorded_utc},
                                    sidecar_error_prefix)
    finally:
        os.close(sidecar_dir_fd)

    if args.kind in _POINTER_ONLY_KINDS:
        record.setdefault("scaffolded", {})[args.kind] = args.src
    record.setdefault("raw_log_paths", {})[args.kind] = _repo_relative(repo_root, sidecar_rel)
    _save_record(repo_root, args.topic, record)

    _emit({
        "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
        "publication_id": args.topic, "kind": args.kind,
        "recorded_path": _repo_relative(repo_root, sidecar_rel), "sha256": sha256, "seq": seq,
    }, 0)


# =============================================================================
# `set-narrative` subcommand (hybrid-publisher step 4: Sonnet narrative is ALWAYS an explicit
# external file argument -- there is no code path here that synthesizes prose on its own).
# =============================================================================

NARRATIVE_KINDS = ("plan", "devlog", "testlog", "report")

_AUTHOR_MAX_LEN = 200
_COMMENT_BREAKING_SUBSTRINGS = ("-->", "<!--")


def _reject_html_comment_breaking_chars(value: str, code_prefix: str, description: str) -> None:
    """P2-FINAL-04: shared guard for any value interpolated verbatim into the provenance HTML
    comment (`<!-- provenance: author=... generated_utc=... source=... -->`). Rejects CR/LF and
    other control characters (which could inject a fabricated extra line right after the real
    provenance line) and the literal comment delimiters '-->'/'<!--' (which could terminate the
    real comment early or open a forged one) BEFORE the value is trusted anywhere -- this runs
    ahead of any scaffold read or record load, so a rejected value never reaches a write path."""
    if any(ord(ch) < 0x20 or ord(ch) == 0x7f for ch in value):
        _emit(_bare_error(f"{code_prefix}_CONTAINS_CONTROL_CHARACTER",
                          f"{description} must not contain control characters or newlines (would "
                          f"corrupt the provenance HTML comment onto more than one physical line)"), 2)
    for forbidden in _COMMENT_BREAKING_SUBSTRINGS:
        if forbidden in value:
            _emit(_bare_error(f"{code_prefix}_CONTAINS_COMMENT_DELIMITER",
                              f"{description} must not contain {forbidden!r} (would terminate or "
                              f"forge an HTML comment inside the provenance header)"), 2)


def _validate_author_or_die(author) -> None:
    """CLI-boundary guard for `--author` (P2-FINAL-04): type/nonempty/length, plus the shared
    HTML-comment-breaking-character guard above. A normal, printable author of reasonable length
    (any script/emoji) is completely unaffected. Runs before the target scaffold is even read, so
    a rejected author can never leave narrative_status or the scaffold file half-mutated."""
    if not isinstance(author, str) or not author:
        _emit(_bare_error("NARRATIVE_AUTHOR_INVALID", "--author must be a non-empty string"), 2)
    if len(author) > _AUTHOR_MAX_LEN:
        _emit(_bare_error("NARRATIVE_AUTHOR_TOO_LONG",
                          f"--author must be at most {_AUTHOR_MAX_LEN} characters, got {len(author)}"), 2)
    _reject_html_comment_breaking_chars(author, "NARRATIVE_AUTHOR", "--author")


def cmd_set_narrative(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    _validate_required_utc_timestamp(args.generated_utc, "generated-utc")
    # P2-FINAL-04: --author and --narrative-file (the "source" provenance field) are both
    # interpolated verbatim into the same HTML provenance comment as --generated-utc -- but
    # --generated-utc is already comment-safe by construction (_validate_required_utc_timestamp
    # only accepts the strict 'YYYY-MM-DDTHH:MM:SSZ' digit shape, which cannot contain '-->' or a
    # newline). --author has no other format constraint, so it needs its own explicit guard; the
    # same guard is applied to --narrative-file's raw string as defense in depth, since it too is
    # attacker-controlled CLI input reaching the same comment line (independent of the existing
    # absolute/escape/symlink checks _resolve_src performs on it as a path).
    _validate_author_or_die(args.author)
    _reject_html_comment_breaking_chars(args.narrative_file, "NARRATIVE_SOURCE", "--narrative-file")
    record = _require_record(repo_root, args.topic)

    if args.kind not in NARRATIVE_KINDS:
        _emit(_bare_error("NARRATIVE_UNSUPPORTED_KIND",
                          f"kind must be one of {NARRATIVE_KINDS}, got {args.kind!r}"), 2)

    target_rel = record.get("scaffolded", {}).get(args.kind)
    if not target_rel:
        _emit(_bare_error("NARRATIVE_TARGET_NOT_SCAFFOLDED",
                          f"no scaffolded '{args.kind}' document for this topic -- run `init` first"), 2)
    error_prefix = "NARRATIVE_%s" % args.kind.upper()
    # docs/_evidence/ is never trusted just because the record says so (P2-02): re-validate
    # containment/no-symlink through the same hardened resolver used for read-only evidence.
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        r = gate.resolve_and_stat_evidence(repo_root_fd, repo_root, repo_root, target_rel,
                                            expect_dir=False, capture_content=True)
    finally:
        os.close(repo_root_fd)
    if r["status"] != "ok":
        _emit(_bare_error(f"{error_prefix}_TARGET_UNSAFE_OR_MISSING",
                          f"scaffolded '{args.kind}' path {target_rel!r} is missing or unsafe "
                          f"(status={r['status']}) -- refusing to trust the persisted record"), 2)

    content_bytes, _sha256 = _resolve_src(repo_root, args.narrative_file, "NARRATIVE")
    narrative_text = content_bytes.decode("utf-8")
    if _NARRATIVE_BEGIN in narrative_text or _NARRATIVE_END in narrative_text:
        _emit(_bare_error("NARRATIVE_CONTAINS_RESERVED_MARKER",
                          f"supplied narrative text must not contain the reserved "
                          f"{_NARRATIVE_BEGIN!r}/{_NARRATIVE_END!r} marker strings"), 2)

    scaffold_text = (r["content_bytes"] or b"").decode("utf-8")
    begin_count = scaffold_text.count(_NARRATIVE_BEGIN)
    end_count = scaffold_text.count(_NARRATIVE_END)
    if begin_count != 1 or end_count != 1:
        _emit(_bare_error("NARRATIVE_MARKERS_MALFORMED",
                          f"scaffold file {target_rel!r} must contain exactly one BEGIN and one END "
                          f"narrative marker (found {begin_count} BEGIN, {end_count} END) -- refusing "
                          f"to guess which occurrence is the real one"), 2)
    begin_idx = scaffold_text.find(_NARRATIVE_BEGIN)
    end_idx = scaffold_text.find(_NARRATIVE_END)
    if end_idx < begin_idx:
        _emit(_bare_error("NARRATIVE_MARKERS_MISSING",
                          f"scaffold file {target_rel!r} is missing its narrative markers -- it was not "
                          f"created by evidence_publisher.py init and cannot be safely narrated"), 2)
    replacement = (
        "%s\n"
        "<!-- provenance: author=%s generated_utc=%s source=%s -->\n"
        "%s\n"
        "%s"
    ) % (_NARRATIVE_BEGIN, args.author, args.generated_utc, args.narrative_file, narrative_text.rstrip("\n"), _NARRATIVE_END)
    new_text = scaffold_text[:begin_idx] + replacement + scaffold_text[end_idx + len(_NARRATIVE_END):]

    parts = _validate_prior_path(repo_root, target_rel, ("docs", args.kind), error_prefix)
    write_repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        dir_fd = _safe_mkdir_chain(write_repo_root_fd, parts[:-1], error_prefix)
    finally:
        os.close(write_repo_root_fd)
    try:
        _safe_replace_file(dir_fd, parts[-1], new_text.encode("utf-8"), error_prefix)
    finally:
        os.close(dir_fd)

    record.setdefault("narrative_status", {})[args.kind] = {
        "status": "authored", "author": args.author, "generated_utc": args.generated_utc,
        "source": args.narrative_file,
    }
    _save_record(repo_root, args.topic, record)

    _emit({
        "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
        "publication_id": args.topic, "kind": args.kind, "path": target_rel,
        "author": args.author, "status": "authored",
    }, 0)


# =============================================================================
# `record-capacity-rejection` subcommand (capacity_rejection's raw gate_evidence pointer -- a
# top-level `capacity_rejection` block, structurally distinct from the `evidence.*` dict).
# =============================================================================

def cmd_record_capacity_rejection(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    _validate_required_utc_timestamp(args.recorded_utc, "recorded-utc")
    record = _require_record(repo_root, args.topic)

    if record.get("task_class") != "capacity_rejection":
        _emit(_bare_error("CAPACITY_REJECTION_WRONG_TASK_CLASS",
                          f"topic {args.topic!r} was initialized with task_class="
                          f"{record.get('task_class')!r}, not 'capacity_rejection'"), 2)

    gate_content, _gate_sha256 = _resolve_src(
        repo_root, args.gate_evidence_src, "CAPACITY_REJECTION_GATE_EVIDENCE")
    gate_basename = f"{args.topic}_capacity_gate.txt"
    _write_evidence_dir_file(repo_root, gate_basename, gate_content,
                              "CAPACITY_REJECTION_GATE_EVIDENCE")

    record["capacity_rejection"] = {
        "rejected": True, "gate_evidence": gate_basename, "reason": args.reason,
        "recorded_utc": args.recorded_utc,
    }
    _save_record(repo_root, args.topic, record)

    _emit({
        "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
        "publication_id": args.topic, "gate_evidence": gate_basename, "rejected": True,
    }, 0)


# =============================================================================
# `publish-benchmark` subcommand -- the ONLY path that ever produces bench_report/certificate
# evidence. FAIL always publishes the human bench_report and REJECTS a supplied certificate
# outright (certificates are PASS-only by contract). PASS publishes a certificate ONLY when the
# caller supplies a real, parseable flat-certificate artifact -- verdict=PASS alone is never
# sufficient to conjure one (plan_26072506 required behavior B6/B5: never synthesize a
# certificate from the manifest's own claims).
# =============================================================================

def _identity_to_bench_meta(identity: dict) -> dict:
    return {"model": identity.get("model"), "gpu_key": identity.get("gpu"), "vllm_version": identity.get("vllm")}


def _publish_dated_kind_no_prefix(repo_root: Path, kind: str, meta: dict,
                                   generated_utc, content_bytes: bytes, ext: str, prior_rel_path):
    """Shared helper for bench_report/benchmark(certificate) under docs/benchmark/: republishing
    the SAME topic reuses its previously recorded filename (overwrite-in-place, through the same
    symlink-rejecting dir_fd machinery as every other publisher-controlled write -- the persisted
    path is re-validated, never trusted); a topic publishing for the first time computes a fresh,
    collision-checked name via doc_naming.bench_filename (NamingCollisionExhausted propagates to
    the caller, which maps it to a stable reason code)."""
    error_prefix = "PUBLISH_BENCHMARK_%s" % kind.upper()
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        if prior_rel_path:
            parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", "benchmark"), error_prefix)
            dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
            try:
                basename = parts[-1]
                _safe_replace_file(dir_fd, basename, content_bytes, error_prefix)
            finally:
                os.close(dir_fd)
            return prior_rel_path
        dir_fd = _safe_mkdir_chain(repo_root_fd, ["docs", "benchmark"], error_prefix)
        try:
            existing = _safe_listdir_names(dir_fd)
            try:
                basename = doc_naming.bench_filename(kind, meta, generated_utc, existing_basenames=existing, ext=ext)
            except doc_naming.NamingCollisionExhausted as e:
                _emit(_bare_error(f"{error_prefix}_COLLISION_EXHAUSTED", str(e)), 2)
            _safe_replace_file(dir_fd, basename, content_bytes, error_prefix)
        finally:
            os.close(dir_fd)
        return "docs/benchmark/%s" % basename
    finally:
        os.close(repo_root_fd)


def _plan_dated_kind_destination(repo_root: Path, kind: str, meta: dict, generated_utc, ext: str,
                                  prior_rel_path, error_prefix: str) -> str:
    """P2-FINAL-05 PLAN phase (validate-only, writes nothing): resolves and validates the exact
    destination `_publish_dated_kind_no_prefix` will later write to, WITHOUT writing any content
    bytes yet. Republishing the same topic re-validates (never re-trusts) its previously recorded
    path via _validate_prior_path; a first-time publish computes a fresh, collision-checked name
    against the real directory listing -- the identical decision _publish_dated_kind_no_prefix
    itself makes, so the later commit-phase call is guaranteed to land on this same path. A
    tampered/escaping prior path or a naming-collision exhaustion is rejected HERE, before the
    caller writes a single byte of report/certificate/record content."""
    if prior_rel_path:
        _validate_prior_path(repo_root, prior_rel_path, ("docs", "benchmark"), error_prefix)
        return prior_rel_path
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        dir_fd = _safe_mkdir_chain(repo_root_fd, ["docs", "benchmark"], error_prefix)
    finally:
        os.close(repo_root_fd)
    try:
        existing = _safe_listdir_names(dir_fd)
    finally:
        os.close(dir_fd)
    try:
        basename = doc_naming.bench_filename(kind, meta, generated_utc, existing_basenames=existing, ext=ext)
    except doc_naming.NamingCollisionExhausted as e:
        _emit(_bare_error(f"{error_prefix}_COLLISION_EXHAUSTED", str(e)), 2)
    return "docs/benchmark/%s" % basename


def _plan_remove_certificate(repo_root: Path, prior_rel_path: str) -> None:
    """P2-FINAL-05 PLAN phase (validate-only, unlinks nothing): confirms `prior_rel_path` is safe
    to remove -- performs the EXACT same checks _remove_prior_certificate performs immediately
    before it unlinks (repo-contained, under docs/benchmark/, and, if present, a plain regular
    file -- never a symlink or directory) -- but never touches the filesystem beyond a read-only
    stat. Used so an unsafe prior certificate pointer (tampered to escape the repo, or swapped for
    a symlink) is rejected here, before the bench_report this call is about to publish alongside
    it has been written -- report/certificate/record must reject together, not one after another."""
    error_prefix = "PUBLISH_BENCHMARK_CERTIFICATE_CLEANUP"
    parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", "benchmark"), error_prefix)
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
    finally:
        os.close(repo_root_fd)
    try:
        kind_of = _safe_stat_in_dir(dir_fd, parts[-1])
        if kind_of not in ("missing", "file"):
            _emit(_bare_error(f"{error_prefix}_TARGET_UNSAFE",
                              f"prior certificate path {prior_rel_path!r} is not a plain regular "
                              f"file (kind={kind_of!r}) -- refusing to unlink it"), 2)
    finally:
        os.close(dir_fd)


def _remove_prior_certificate(repo_root: Path, prior_rel_path: str) -> None:
    """P2-A02: on a PASS->FAIL/REFUTE transition, a certificate published by an earlier PASS run
    of this same topic must not keep being associated with the new, contradictory verdict. Only a
    re-validated (repo-relative, under docs/benchmark/, no '..' escape), lstat-confirmed REGULAR
    FILE is ever unlinked -- a persisted record path is never trusted as-is (matches the same
    _validate_prior_path/_safe_stat_in_dir discipline every other publisher-controlled write path
    uses). A symlink, directory, or escaping path fails closed (stable JSON, exit 2) rather than
    ever unlinking through it; an already-missing file is a no-op (nothing to clean up)."""
    error_prefix = "PUBLISH_BENCHMARK_CERTIFICATE_CLEANUP"
    parts = _validate_prior_path(repo_root, prior_rel_path, ("docs", "benchmark"), error_prefix)
    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        dir_fd = _safe_mkdir_chain(repo_root_fd, parts[:-1], error_prefix)
    finally:
        os.close(repo_root_fd)
    try:
        basename = parts[-1]
        kind_of = _safe_stat_in_dir(dir_fd, basename)
        if kind_of == "missing":
            return
        if kind_of != "file":
            _emit(_bare_error(f"{error_prefix}_TARGET_UNSAFE",
                              f"prior certificate path {prior_rel_path!r} is not a plain regular "
                              f"file (kind={kind_of!r}) -- refusing to unlink it"), 2)
        os.unlink(basename, dir_fd=dir_fd)
    finally:
        os.close(dir_fd)


# ---- rubric carrier for non-PASS runs (plan_26082405) ----------------------------------------
# 인증서는 PASS 때만 발행되므로 REFUTE 런의 루브릭 사실(어느 권한에서 쟀나 · 문턱은 얼마였나)은
# 인증서를 통해 승격 판정기에 **도달할 수 없다** — 그래서 completion_gate 의 explore 자동개방이
# REFUTE 에서 죽은 코드였다. 여기서 판정기 산출물(verdict_rule.py JSON)을 직접 읽어 record 의
# benchmark 오브젝트에 싣는다. finalize 가 그 오브젝트를 그대로 work-manifest 로 옮기므로,
# certificate 가 null 이어도 authority 는 살아있는 채널로 게이트에 도달한다.
# ★ 값을 **만들지 않는다** — 판정기가 계산한 것을 옮길 뿐이고, 출처는 rubric_source 로 표시한다.
RUBRIC_RECORD_FIELDS = ("rubric_authority", "floor_tps", "ratio_M_over_primary",
                        "primary_source", "rubric_source")


def _rubric_from_verdict_json(repo_root: Path, verdict_json_src: str, cli_verdict: str,
                              error_prefix: str) -> dict:
    """verdict_rule.py 산출물에서 rubric 사실을 파생한다(합성 ✗ · fail-closed).

    거부(exit 2)하는 것은 **모순**뿐이다 — 파싱 불가, 또는 산출물의 verdict 가 --verdict 와
    다름(인증서의 `_VERDICT_MISMATCH` 와 같은 규율: 서로 어긋나는 아티팩트를 발행하지 않는다),
    또는 authority 가 값역 밖. 반면 rubric 수치의 **결측은 거부하지 않는다** — 판정기가 루브릭을
    못 세운 런(NEEDS_RUBRIC 계열)이 실제로 존재하고, 그 사실은 null 로 정직하게 기록되어
    승격 판정기에서 fail-closed 로 막히는 것이 옳다(여기서 막으면 리포트 발행까지 함께 죽는다).
    """
    content_bytes, _sha = _resolve_src(repo_root, verdict_json_src, error_prefix)
    try:
        doc = json.loads(content_bytes.decode("utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError) as e:
        _emit(_bare_error(f"{error_prefix}_UNPARSEABLE",
                          f"--verdict-json-src {verdict_json_src!r} is not parseable JSON: {e} -- "
                          f"refusing to derive rubric facts from an unreadable artifact"), 2)
    if not isinstance(doc, dict):
        _emit(_bare_error(f"{error_prefix}_WRONG_SHAPE",
                          f"--verdict-json-src {verdict_json_src!r} must contain a JSON object, "
                          f"got {type(doc).__name__}"), 2)
    doc_verdict = doc.get("verdict")
    if doc_verdict != cli_verdict:
        _emit(_bare_error(f"{error_prefix}_VERDICT_MISMATCH",
                          f"verdict JSON says verdict={doc_verdict!r} but --verdict={cli_verdict!r} "
                          f"-- refusing to record rubric facts from a contradictory artifact"), 2)
    rub = doc.get("rubric")
    rub = rub if isinstance(rub, dict) else {}
    raw_authority = rub.get("authority")
    authority = raw_authority.strip() if isinstance(raw_authority, str) else None
    if authority and authority not in gate.RUBRIC_AUTHORITIES:
        _emit(_bare_error(f"{error_prefix}_AUTHORITY_UNKNOWN",
                          f"verdict JSON rubric.authority={raw_authority!r} is not one of "
                          f"{list(gate.RUBRIC_AUTHORITIES)} -- fail closed rather than record an "
                          f"authority no consumer can reason about"), 2)

    def _number(value):
        return value if isinstance(value, (int, float)) and not isinstance(value, bool) else None

    primary_source = rub.get("source")
    primary_source = primary_source.strip() if isinstance(primary_source, str) else None
    return {
        "rubric_authority": authority or None,
        "floor_tps": _number(rub.get("floor")),
        "ratio_M_over_primary": _number(rub.get("ratio_M_over_primary")),
        "primary_source": primary_source or None,
        # 출처 표시(헌법 §결정론 규율): 이 네 값이 판정기 산출물 파생분임을 데이터가 스스로 밝힌다.
        "rubric_source": "verdict_json",
    }


def cmd_publish_benchmark(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    _validate_required_utc_timestamp(args.generated_utc, "generated-utc")
    record = _require_record(repo_root, args.topic)

    if record.get("task_class") != "full_benchmark":
        _emit(_bare_error("PUBLISH_BENCHMARK_WRONG_TASK_CLASS",
                          f"topic {args.topic!r} was initialized with task_class="
                          f"{record.get('task_class')!r}, not 'full_benchmark'"), 2)

    if args.verdict != "PASS" and args.certificate_src:
        _emit(_bare_error("PUBLISH_BENCHMARK_CERTIFICATE_WITHOUT_PASS",
                          f"--certificate-src was supplied but --verdict={args.verdict!r} != 'PASS' -- "
                          f"certificates are published PASS-only (matches work-manifest.schema.json's "
                          f"own CERTIFICATE_PRESENT_WITHOUT_PASS_VERDICT structural contract)"), 2)

    identity = record.get("identity") or {}
    meta = _identity_to_bench_meta(identity)
    scaffolded_before = record.get("scaffolded", {})
    report_error_prefix = "PUBLISH_BENCHMARK_BENCH_REPORT"
    cert_error_prefix = "PUBLISH_BENCHMARK_CERTIFICATE"

    # =========================================================================
    # PLAN PHASE (P2-FINAL-05): every source, destination, and prior-state cleanup is validated
    # here FIRST -- nothing under docs/benchmark/ or docs/_evidence/ is written, overwritten, or
    # unlinked until every check below has already passed. A rejection anywhere in this phase
    # leaves the bench_report, certificate, and persisted record bytes ALL exactly as they were
    # before this command ran: no orphan new report, no report/record contradiction.
    # =========================================================================
    report_bytes, _sha = _resolve_src(repo_root, args.bench_report_src, report_error_prefix)
    rubric_record = {field: None for field in RUBRIC_RECORD_FIELDS}
    if args.verdict_json_src:
        rubric_record = _rubric_from_verdict_json(
            repo_root, args.verdict_json_src, args.verdict, "PUBLISH_BENCHMARK_VERDICT_JSON")
    _plan_dated_kind_destination(
        repo_root, "bench_report", meta, args.generated_utc, "md",
        scaffolded_before.get("bench_report"), report_error_prefix,
    )

    prior_cert_rel = scaffolded_before.get("certificate")
    clear_prior_certificate = args.verdict != "PASS" and bool(prior_cert_rel)
    if clear_prior_certificate:
        # P2-A02: a FAIL/REFUTE republish of a topic that previously PASSed with a certificate
        # must not leave that certificate silently still associated with the new, contradictory
        # verdict. P2-FINAL-05: this safety check now runs BEFORE the bench_report write below,
        # not after it -- an unsafe prior certificate pointer must reject the whole transition,
        # not just the certificate half of it.
        _plan_remove_certificate(repo_root, prior_cert_rel)

    cert_bytes = None
    cert_dest_rel = None
    if args.verdict == "PASS" and args.certificate_src:
        cert_bytes, _csha = _resolve_src(repo_root, args.certificate_src, cert_error_prefix)
        try:
            cert_text = cert_bytes.decode("utf-8")
        except UnicodeDecodeError:
            cert_text = None
        fields, parse_ok = gate.parse_flat_certificate(cert_text) if cert_text is not None else ({}, False)
        if not parse_ok:
            _emit(_bare_error(f"{cert_error_prefix}_UNPARSEABLE",
                              f"--certificate-src {args.certificate_src!r} is not a recognized flat-scalar "
                              f"certificate artifact -- refusing to publish it (never publish an unverifiable "
                              f"file as if it were a real certificate)"), 2)
        # P2-04: verdict=PASS on the CLI is not enough by itself -- the certificate's OWN content
        # must actually say PASS/full and match this publication's recorded strong identity.
        # Reuses completion_gate's own field map/comparison (never a second, divergent copy).
        if fields.get("verdict") != "PASS":
            _emit(_bare_error(f"{cert_error_prefix}_VERDICT_MISMATCH",
                              f"certificate verdict={fields.get('verdict')!r} (must be 'PASS') -- "
                              f"refusing to publish a contradictory certificate"), 2)
        if fields.get("benchmark_mode") != "full":
            _emit(_bare_error(f"{cert_error_prefix}_MODE_MISMATCH",
                              f"certificate benchmark_mode={fields.get('benchmark_mode')!r} (must be 'full') -- "
                              f"refusing to publish a contradictory certificate"), 2)
        _mapped, mismatched = gate._certificate_strong_identity_matches(fields, identity)
        if mismatched:
            _emit(_bare_error(f"{cert_error_prefix}_IDENTITY_MISMATCH",
                              f"certificate identity field(s) {mismatched} do not match this "
                              f"publication's recorded identity -- refusing to publish a mismatched certificate"), 2)
        cert_dest_rel = _plan_dated_kind_destination(
            repo_root, "benchmark", meta, args.generated_utc, "yaml",
            scaffolded_before.get("certificate"), cert_error_prefix,
        )

    # =========================================================================
    # COMMIT PHASE: everything above already validated -- nothing here is expected to reject.
    # Each write/removal independently re-validates (never trusts a plan result across time) and
    # is itself atomic (temp-file + os.replace, or a re-validated unlink). The persisted record is
    # saved LAST, so it only ever describes a bench_report/certificate state that has already
    # fully landed on disk.
    # =========================================================================
    bench_report_rel = _publish_dated_kind_no_prefix(
        repo_root, "bench_report", meta, args.generated_utc, report_bytes, "md",
        scaffolded_before.get("bench_report"),
    )
    scaffolded = record.setdefault("scaffolded", {})
    scaffolded["bench_report"] = bench_report_rel

    if clear_prior_certificate:
        _remove_prior_certificate(repo_root, prior_cert_rel)
        scaffolded["certificate"] = None

    certificate_rel = scaffolded.get("certificate") if args.verdict == "PASS" else None
    pending_certificate = args.verdict == "PASS"
    if cert_dest_rel:
        certificate_rel = _publish_dated_kind_no_prefix(
            repo_root, "benchmark", meta, args.generated_utc, cert_bytes, "yaml",
            scaffolded_before.get("certificate"),
        )
        scaffolded["certificate"] = certificate_rel
        pending_certificate = False

    # rubric 은 mode/verdict 와 **같은 커밋**에 실린다 — 갈라지면 finalize 가 나르는 오브젝트가
    # 이 발행의 verdict 와 다른 런의 루브릭을 섞어 담을 수 있다(carrier 불일치 = 이 결함 계열).
    record["benchmark"] = {"mode": "full", "verdict": args.verdict, **rubric_record}
    # Keep the persisted producer-derived contract canonical in the same commit as the verdict.
    # Otherwise an init-without-verdict -> publish PASS transition would add certificate to the
    # required matrix semantically while leaving required_evidence stale, making the next load
    # reject the publisher's own record.
    record["required_evidence"] = required_evidence_for(
        record["task_class"], record["conditions"], args.verdict)
    record["or_group_scaffolded"] = (
        list(CAPACITY_REJECTION_OR_GROUP)
        if record["task_class"] == "capacity_rejection" else [])
    record["capacity_rejection_required"] = record["task_class"] == "capacity_rejection"
    _save_record(repo_root, args.topic, record)

    _emit({
        "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
        "publication_id": args.topic, "verdict": args.verdict,
        "bench_report_path": bench_report_rel, "certificate_path": certificate_rel,
        "pending_certificate": pending_certificate,
        "rubric": dict(rubric_record),
    }, 0)


# =============================================================================
# `finalize` subcommand -- builds a work-manifest from the publication record and DELEGATES its
# entire schema/identity/link/PII/verdict verification to the constitution completion gate (never a
# second, reimplemented copy of those checks). Relays completion_gate's own stable JSON and exit
# code verbatim, so its output stays valid against completion-manifest.schema.json.
# =============================================================================

_ALL_EVIDENCE_KEYS = ("plan", "devlog", "testlog", "simlog", "bench_report", "certificate",
                      "verification", "commit", "report")


def cmd_finalize(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    record = _require_record(repo_root, args.topic)

    pii_scan = _load_json(args.pii_scan_json, "pii_scan") if args.pii_scan_json else {"passed": False}
    runtime = _load_json(args.runtime_json, "runtime") if args.runtime_json else None
    capacity_rejection = _load_json(args.capacity_rejection_json, "capacity_rejection") \
        if args.capacity_rejection_json else record.get("capacity_rejection")

    manifest_dir = repo_root / "docs" / "_evidence"  # pure path arithmetic below -- no I/O here
    scaffolded = record.get("scaffolded", {})
    narrative_status = record.get("narrative_status", {})
    evidence = {}
    finalize_repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        for key in _ALL_EVIDENCE_KEYS:
            rel = scaffolded.get(key)
            if not rel:
                evidence[key] = None
                continue
            # P2-A01: for the narrative-bearing doc kinds (plan/devlog/testlog/report), a
            # scaffold file merely existing on disk is NOT "existing required evidence" -- it
            # still holds the deterministic `_PLACEHOLDER` scaffold text until `set-narrative`
            # explicitly authors it (narrative_status[kind].status == "authored"). Direct file
            # tampering (hand-editing the placeholder text away without ever calling
            # set-narrative) must not substitute for that -- so this checks BOTH the persisted
            # record's authored status AND that the actual on-disk content no longer contains the
            # placeholder sentinel (re-read through the same hardened, symlink-rejecting resolver
            # every other evidence path goes through, never trusted path-only). When either check
            # fails, this evidence key is treated as absent (None) so completion_gate.py's own,
            # already-delegated-to EVIDENCE_MISSING:<key> logic is what blocks the state --
            # finalize still never self-judges identity/link/PII/verdict/promotion.
            if key in NARRATIVE_KINDS:
                if narrative_status.get(key, {}).get("status") != "authored":
                    evidence[key] = None
                    continue
                r = gate.resolve_and_stat_evidence(finalize_repo_root_fd, repo_root, repo_root, rel,
                                                    expect_dir=False, capture_content=True)
                if r["status"] != "ok":
                    evidence[key] = None
                    continue
                try:
                    text = (r["content_bytes"] or b"").decode("utf-8")
                except UnicodeDecodeError:
                    evidence[key] = None
                    continue
                if _PLACEHOLDER in text:
                    evidence[key] = None
                    continue
            evidence[key] = {"path": os.path.relpath(str(repo_root / rel), str(manifest_dir))}
    finally:
        os.close(finalize_repo_root_fd)

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_class": record["task_class"],
        "identity": record["identity"],
        "evidence": evidence,
        "conditions": record.get("conditions") or {},
        "benchmark": record.get("benchmark") or {"mode": None, "verdict": None},
        "pii_scan": pii_scan,
    }
    if record["task_class"] == "capacity_rejection":
        manifest["capacity_rejection"] = capacity_rejection
    elif runtime is not None:
        manifest["runtime"] = runtime

    # Fail closed before persisting the producer artifact. completion_gate remains the sole
    # contract implementation: this calls its generic schema engine against the exact manifest
    # that would otherwise be written, rather than duplicating PII/runtime/capacity rules here.
    preflight_errors = gate.validate_against_schema(
        manifest, gate.WORK_SCHEMA, gate.WORK_SCHEMA, "$")
    if preflight_errors:
        details = "; ".join(f"{path}: {message}" for path, message in preflight_errors)
        _emit(_bare_error("FINALIZE_WORK_MANIFEST_SCHEMA_INVALID", details), 2)

    manifest_basename = "%s.work-manifest.json" % args.topic
    manifest_bytes = json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2).encode("utf-8")
    _write_evidence_dir_file(repo_root, manifest_basename, manifest_bytes, "WORK_MANIFEST")
    manifest_path = manifest_dir / manifest_basename

    proc = subprocess.run(
        [sys.executable, str(_SCRIPTS_DIR / "completion_gate.py"), "verify",
         "--manifest", str(manifest_path), "--repo-root", str(repo_root)],
        capture_output=True, text=True, timeout=60,
    )
    sys.stdout.write(proc.stdout)
    raise SystemExit(proc.returncode)


# =============================================================================
# `init` subcommand
# =============================================================================

def cmd_init(args: argparse.Namespace) -> None:
    repo_root = _resolve_repo_root(args.repo_root)
    _validate_topic_or_die(args.topic)
    _validate_required_utc_timestamp(args.generated_utc, "generated-utc")
    identity = _load_json(args.identity_json, "identity")
    if not isinstance(identity, dict):
        _emit(_bare_error("INIT_IDENTITY_NOT_AN_OBJECT",
                          f"--identity-json must contain a JSON object, got {type(identity).__name__}"), 2)
    conditions = _load_json(args.conditions_json, "conditions") if args.conditions_json else {}
    if not isinstance(conditions, dict):
        _emit(_bare_error("INIT_CONDITIONS_NOT_AN_OBJECT",
                          f"--conditions-json must contain a JSON object, got {type(conditions).__name__}"), 2)
    identity_errors = gate.validate_against_schema(
        identity, gate.WORK_SCHEMA["definitions"]["identity"], gate.WORK_SCHEMA, "$.identity")
    conditions_errors = gate.validate_against_schema(
        conditions, gate.WORK_SCHEMA["properties"]["conditions"], gate.WORK_SCHEMA, "$.conditions")
    if identity_errors or conditions_errors:
        details = [f"{path}: {message}" for path, message in identity_errors + conditions_errors]
        _emit(_bare_error("INIT_INPUT_SCHEMA_INVALID", "; ".join(details)), 2)

    task_class_choices = gate.WORK_SCHEMA["properties"]["task_class"]["enum"]
    if args.task_class not in task_class_choices:
        _emit(_bare_error("INIT_UNSUPPORTED_TASK_CLASS",
                          f"task_class must be one of {task_class_choices}, got {args.task_class!r}"), 2)

    prior = _load_record(repo_root, args.topic) or {}
    prior_benchmark_value = prior.get("benchmark")
    prior_benchmark_for_required = prior_benchmark_value if isinstance(prior_benchmark_value, dict) else {}
    verdict = (args.benchmark_verdict if args.benchmark_verdict is not None
               else prior_benchmark_for_required.get("verdict"))
    required = required_evidence_for(args.task_class, conditions, verdict)

    or_group_scaffolded = list(CAPACITY_REJECTION_OR_GROUP) if args.task_class == "capacity_rejection" else []
    capacity_rejection_required = args.task_class == "capacity_rejection"

    if "report" in required and not args.report_slug:
        _emit(_bare_error("INIT_REPORT_SLUG_REQUIRED",
                          "task_class/conditions require a 'report' evidence item -- pass --report-slug "
                          "(<분류>_<YYMMDDHH>[_MM_SS]_<한글제목> per docs.md's report/ rule)"), 2)

    if prior:
        immutable = {
            "task_class": args.task_class,
            "generated_utc": args.generated_utc,
            "identity": identity,
            "conditions": conditions,
        }
        mismatches = [field for field, value in immutable.items() if prior.get(field) != value]
        prior_benchmark_for_rebind = prior.get("benchmark")
        if isinstance(prior_benchmark_for_rebind, dict):
            for field, value in (("mode", args.benchmark_mode), ("verdict", args.benchmark_verdict)):
                if value is not None and prior_benchmark_for_rebind.get(field) not in (None, value):
                    mismatches.append(f"benchmark.{field}")
        if mismatches:
            _emit(_bare_error(
                "INIT_IMMUTABLE_REBIND",
                "existing publication identity/time/conditions/benchmark cannot be rebound: "
                + ", ".join(sorted(mismatches))), 2)
    prior_scaffolded = prior.get("scaffolded", {})

    kinds_to_scaffold = [k for k in required if k in SCAFFOLDABLE_KINDS]
    for k in or_group_scaffolded:
        if k not in kinds_to_scaffold:
            kinds_to_scaffold.append(k)

    scaffolded: dict[str, str | None] = {k: prior_scaffolded.get(k) for k in
                                          ("plan", "devlog", "testlog", "simlog", "report",
                                           "bench_report", "certificate", "verification", "commit")}
    created_now: list[str] = []
    already_existed: list[str] = []

    repo_root_fd = _open_repo_root_fd(repo_root)
    try:
        for kind in kinds_to_scaffold:
            prior_path = prior_scaffolded.get(kind)
            if kind in DATED_KINDS:
                rel_path, created = _ensure_dated_scaffold(repo_root_fd, repo_root, kind, args.topic, args.task_class,
                                                            args.generated_utc, prior_path)
            elif kind == "simlog":
                rel_path, created = _ensure_simlog_scaffold(repo_root_fd, repo_root, args.topic, args.generated_utc, prior_path)
            elif kind == "report":
                rel_path, created = _ensure_report_scaffold(repo_root_fd, repo_root, args.topic, args.task_class,
                                                             args.generated_utc, args.report_slug, prior_path)
            else:  # pragma: no cover -- kinds_to_scaffold is built only from SCAFFOLDABLE_KINDS + OR-group
                raise AssertionError(f"unreachable scaffold kind {kind!r}")
            scaffolded[kind] = rel_path
            (created_now if created else already_existed).append(kind)
    finally:
        os.close(repo_root_fd)

    pending_evidence = sorted(set(required) - set(SCAFFOLDABLE_KINDS))

    prior_benchmark = prior.get("benchmark", {"mode": None, "verdict": None})
    if not isinstance(prior_benchmark, dict):
        prior_benchmark = {"mode": None, "verdict": None}
    benchmark = {
        "mode": args.benchmark_mode if args.benchmark_mode is not None else prior_benchmark.get("mode"),
        "verdict": verdict if args.benchmark_verdict is not None else prior_benchmark.get("verdict"),
    }
    # 이미 publish-benchmark 가 실어둔 rubric(판정기 파생분)은 re-init 이 말없이 떨어뜨리면 안 된다 --
    # 그 침묵 손실이 곧 이 결함 계열(통로 끊김)이다. init 은 rubric 을 **만들지 않고** 보존만 한다.
    for field in RUBRIC_RECORD_FIELDS:
        if prior_benchmark.get(field) is not None:
            benchmark[field] = prior_benchmark[field]
    record = {
        "schema_version": SCHEMA_VERSION,
        "publication_id": args.topic,
        "task_class": args.task_class,
        "generated_utc": args.generated_utc,
        "identity": identity,
        "conditions": conditions,
        "benchmark": benchmark,
        "required_evidence": required,
        "or_group_scaffolded": or_group_scaffolded,
        "capacity_rejection_required": capacity_rejection_required,
        "scaffolded": scaffolded,
        "narrative_status": prior.get("narrative_status", {}),
        "raw_log_paths": prior.get("raw_log_paths", {}),
        "capacity_rejection": prior.get("capacity_rejection"),
    }
    record_path = _save_record(repo_root, args.topic, record)

    _emit({
        "schema_version": SCHEMA_VERSION, "ok": True, "reason_codes": [], "messages": {},
        "publication_id": args.topic, "task_class": args.task_class,
        "required_evidence": required,
        "or_group_scaffolded": or_group_scaffolded,
        "capacity_rejection_required": capacity_rejection_required,
        "scaffolded": scaffolded,
        "pending_evidence": pending_evidence,
        "created_now": sorted(created_now),
        "already_existed": sorted(already_existed),
        "record_path": _repo_relative(repo_root, record_path),
    }, 0)


# =============================================================================
# CLI plumbing
# =============================================================================

class _PublisherArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        _emit(_bare_error("CLI_USAGE_ERROR", f"invalid command-line invocation: {message}"), 2)


def _build_parser() -> _PublisherArgumentParser:
    ap = _PublisherArgumentParser(
        description="evidence_publisher.py -- hybrid evidence publisher (plan_26072506 Phase 2)",
    )
    ap.add_argument("--self-test", action="store_true", help="run the built-in self-test and exit")
    sub = ap.add_subparsers(dest="cmd")

    task_class_choices = gate.WORK_SCHEMA["properties"]["task_class"]["enum"]
    bench_mode_choices = [v for v in gate.WORK_SCHEMA["properties"]["benchmark"]["properties"]["mode"]["enum"] if v]
    bench_verdict_choices = [v for v in gate.WORK_SCHEMA["properties"]["benchmark"]["properties"]["verdict"]["enum"] if v]

    p_init = sub.add_parser("init", help="scaffold required evidence + publication record for a topic")
    p_init.add_argument("--repo-root")
    p_init.add_argument("--topic", required=True)
    p_init.add_argument("--task-class", required=True, choices=task_class_choices)
    p_init.add_argument("--generated-utc", required=True)
    p_init.add_argument("--identity-json", required=True)
    p_init.add_argument("--conditions-json")
    p_init.add_argument("--benchmark-mode", choices=bench_mode_choices)
    p_init.add_argument("--benchmark-verdict", choices=bench_verdict_choices)
    p_init.add_argument("--report-slug")
    p_init.set_defaults(func=cmd_init)

    all_evidence_kinds = list(gate.WORK_SCHEMA["properties"]["evidence"]["properties"].keys())

    p_append = sub.add_parser("append-raw", help="accumulate one raw-evidence source (append-only, atomic)")
    p_append.add_argument("--repo-root")
    p_append.add_argument("--topic", required=True)
    # choices intentionally covers ALL evidence kinds (not just APPEND_RAW_KINDS) so that
    # bench_report/certificate produce our own informative APPEND_RAW_UNSUPPORTED_KIND reason
    # code (directing the caller to `publish-benchmark`) rather than a generic argparse usage error.
    p_append.add_argument("--kind", required=True, choices=all_evidence_kinds)
    p_append.add_argument("--src", required=True)
    p_append.add_argument("--recorded-utc", required=True)
    p_append.add_argument("--dest-name")
    p_append.set_defaults(func=cmd_append_raw)

    p_narr = sub.add_parser("set-narrative", help="inject Sonnet-authored narrative (explicit external input only)")
    p_narr.add_argument("--repo-root")
    p_narr.add_argument("--topic", required=True)
    p_narr.add_argument("--kind", required=True, choices=NARRATIVE_KINDS)
    p_narr.add_argument("--narrative-file", required=True)
    p_narr.add_argument("--author", required=True)
    p_narr.add_argument("--generated-utc", required=True)
    p_narr.set_defaults(func=cmd_set_narrative)

    p_cr = sub.add_parser("record-capacity-rejection", help="record the raw gate_evidence pointer for a capacity_rejection topic")
    p_cr.add_argument("--repo-root")
    p_cr.add_argument("--topic", required=True)
    p_cr.add_argument("--gate-evidence-src", required=True)
    p_cr.add_argument("--reason", required=True)
    p_cr.add_argument("--recorded-utc", required=True)
    p_cr.set_defaults(func=cmd_record_capacity_rejection)

    p_bench = sub.add_parser("publish-benchmark", help="publish bench_report (always) and certificate (PASS-only, from a supplied artifact)")
    p_bench.add_argument("--repo-root")
    p_bench.add_argument("--topic", required=True)
    p_bench.add_argument("--verdict", required=True, choices=["PASS", "FAIL", "REFUTE"])
    p_bench.add_argument("--generated-utc", required=True)
    p_bench.add_argument("--bench-report-src", required=True)
    p_bench.add_argument("--certificate-src")
    p_bench.add_argument("--verdict-json-src",
                         help="verdict_rule.py 산출물(JSON, repo-relative). rubric authority/floor/"
                              "ratio/primary_source 를 record.benchmark 로 옮겨 REFUTE 런에서도 "
                              "승격 판정기가 루브릭 권한을 읽게 한다(인증서는 PASS 전용이라 "
                              "REFUTE 의 carrier 가 못 된다 — plan_26082405).")
    p_bench.set_defaults(func=cmd_publish_benchmark)

    p_fin = sub.add_parser("finalize", help="build a work-manifest from the publication record and delegate to completion_gate.py verify")
    p_fin.add_argument("--repo-root")
    p_fin.add_argument("--topic", required=True)
    p_fin.add_argument("--pii-scan-json")
    p_fin.add_argument("--runtime-json")
    p_fin.add_argument("--capacity-rejection-json")
    p_fin.set_defaults(func=cmd_finalize)

    return ap


def _self_test() -> None:
    """Exercises a real init -> append-raw -> finalize roundtrip against a hermetic temp repo
    (in-process, capturing stdout/SystemExit rather than shelling out to itself) and asserts the
    result reaches the expected completion-gate state. `finalize` still genuinely subprocess-
    invokes the real constitution completion_gate.py -- this is not a mocked check."""
    import contextlib
    import io

    def invoke(argv):
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf):
                main(argv)
            code = 0
        except SystemExit as e:
            code = e.code if isinstance(e.code, int) else 0
        text = buf.getvalue()
        return code, (json.loads(text) if text.strip() else None)

    with tempfile.TemporaryDirectory() as td:
        repo_root = Path(td)
        for d in ("plan", "devlog", "testlog", "simlog", "benchmark", "report"):
            (repo_root / "docs" / d).mkdir(parents=True, exist_ok=True)

        identity = {"model": "self-test-model", "gpu": "GB10", "vllm": "0.0.0",
                    "quant": None, "topology": "single", "tp": 1}
        identity_path = repo_root / "identity.json"
        identity_path.write_text(json.dumps(identity), encoding="utf-8")

        code, out = invoke(["init", "--repo-root", str(repo_root), "--topic", "selftest",
                            "--task-class", "minor_patch", "--generated-utc", "2026-01-01T00:00:00Z",
                            "--identity-json", str(identity_path)])
        if not (code == 0 and isinstance(out, dict) and out.get("ok")):
            raise RuntimeError(f"self-test init failed: {out!r}")
        if sorted(out["required_evidence"]) != ["commit", "verification"]:
            raise RuntimeError(f"self-test required evidence mismatch: {out!r}")

        (repo_root / "verify.txt").write_text("py_compile OK\n", encoding="utf-8")
        (repo_root / "commit.txt").write_text("deadbeef\n", encoding="utf-8")
        for kind, src in (("verification", "verify.txt"), ("commit", "commit.txt")):
            code, out = invoke(["append-raw", "--repo-root", str(repo_root), "--topic", "selftest",
                                "--kind", kind, "--src", src, "--recorded-utc", "2026-01-01T00:01:00Z"])
            if not (code == 0 and isinstance(out, dict) and out.get("ok")):
                raise RuntimeError(f"self-test append-raw failed for {kind}: {out!r}")

        # evidence.*.path is documented as relative to the work-manifest file's OWN directory
        # (docs/_evidence/), not repo-root -- scanned_paths must match those exact strings.
        manifest_dir = repo_root / "docs" / "_evidence"
        pii_path = repo_root / "pii.json"
        pii_path.write_text(json.dumps({"passed": True, "scanned_paths": [
            os.path.relpath(str(repo_root / "verify.txt"), str(manifest_dir)),
            os.path.relpath(str(repo_root / "commit.txt"), str(manifest_dir)),
        ]}), encoding="utf-8")
        runtime_path = repo_root / "runtime.json"
        runtime_path.write_text(json.dumps({
            "health_ok": True, "functional_smoke_passed": True, "identity": identity,
            "containers": [{"name": "svc", "restart_count": 0, "oom_killed": False}],
        }), encoding="utf-8")

        code, out = invoke(["finalize", "--repo-root", str(repo_root), "--topic", "selftest",
                            "--pii-scan-json", str(pii_path), "--runtime-json", str(runtime_path)])
        if not isinstance(out, dict) or out.get("state") != "evidence-complete":
            raise RuntimeError(f"self-test finalize state mismatch: {out!r}")
        if out.get("eligible_for_promotion") is not False:  # minor_patch is promotion-capped
            raise RuntimeError(f"self-test promotion cap mismatch: {out!r}")

    print("[evidence_publisher] self-test PASS")


def main(argv=None) -> None:
    ap = _build_parser()
    args = ap.parse_args(argv)
    if args.self_test:
        _self_test()
        return
    if not args.cmd:
        _emit(_bare_error("CLI_USAGE_ERROR", "no subcommand given (init|append-raw|set-narrative|publish-benchmark|finalize)"), 2)
    args.func(args)


if __name__ == "__main__":
    main()
