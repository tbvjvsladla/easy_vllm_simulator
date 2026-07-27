#!/usr/bin/env python3
"""hint_tag.py — manage hint/<vllm>/<model>/<arch> recipe-hint tags.

Design: docs/plan/plan_26070222. The deployment "hint" layer distributes
distilled serving-recipe KNOWLEDGE as annotated git tags — never finished products.

[A]=B residence: the recipe body lives in the tag ANNOTATION. HEAD stays a pure
skeleton+engine (no recipe files at HEAD); HEAD carries only an index (HINTS.md
카탈로그 + hints/index.json). Retrieved by `git fetch --tags` + `git show <tag>`.

Deterministic here (script) / judgment there (agent):
  - validates names (shape + `git check-ref-format`), one canonical model-slug/family,
  - scaffolds from resolved.json (surgical scalar extraction — resolved.json itself
    carries operator PII in some notes, so only clean fields are pulled),
  - PII-scans FAIL-CLOSED over the FULL recipe body AND the tagger identity that ships
    inside the pushed tag object (pii_terms.txt literals — single source shared with
    scan_forbidden_strings.py — plus generic private-IP/email/path/host patterns),
  - enforces the B1 backstop (absolute host-scoped numbers require a re-measure caveat),
  - tags, indexes (index.json + HINTS.md row), verifies, and pushes ONLY refs/tags/hint/*
    (never --tags, which would leak local last-good-* rollback anchors to a public origin).
The agent authors the judgment slots (context / wall-map / serve-knob why / re-verify).

Subcommands: create · finalize · verify · push · match · reverify.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
from datetime import date
from pathlib import Path

TAG_SHAPE = re.compile(r"^hint/[^/]+/[^/]+/[^/]+$")
HINTS_MARKER = "<!-- hint-index:rows -->"

# Canonical model-slug per family (hardening #5 — prevents slug sprawl that would break
# `git tag -l 'hint/*/<slug>/*'`). Value = accepted spellings (canonical MUST be first-listed
# via the dict key). Register a new family here (HITL) rather than minting drive-by slugs.
CANONICAL_SLUGS: dict[str, set[str]] = {
    "deepseek-v4-flash": {"deepseek-v4-flash", "ds4flash", "deepseek-v4-flash-dspark"},
    "gpt-oss-120b": {"gpt-oss-120b", "gptoss120b", "gpt-oss"},
    "qwen3-next-80b-bf16": {"qwen3-next-80b-bf16", "qwen3next80b", "qwen3-next-80b"},
    "qwen3.5-122b-a10b-nvfp4": {"qwen3.5-122b-a10b-nvfp4", "qwen35-122b-nvfp4"},
    "skt-a.x-4.0-72b": {"skt-a.x-4.0-72b", "skt-ax-72b"},
    "gemma-3-27b": {"gemma-3-27b", "gemma3-27b"},
}

# Generic PII patterns (belt-and-suspenders atop pii_terms.txt literals). Narrow on
# purpose so versions ("2.11.0" = 3 octets) don't false-positive as IPv4.
GENERIC_PII: list[tuple[str, re.Pattern]] = [
    ("private-ipv4", re.compile(r"\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}(?:\.\d{1,3})?")),
    ("email", re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")),
    ("abs-op-path", re.compile(r"/(?:mnt|home)/[A-Za-z0-9._/-]+")),
    ("spark-host", re.compile(r"spark-[0-9a-f]{3,}")),
]


def die(msg: str) -> "NoReturn":  # noqa: F821
    print(msg, file=sys.stderr)
    raise SystemExit(1)


def repo_root() -> Path:
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    except OSError:
        out = None
    if out is not None and out.returncode == 0:
        return Path(out.stdout.strip())
    # Gitless release archives still ship the read-only hints/index.json consumer.  Resolve that
    # distribution root from this owner-local script, without weakening mutation commands below.
    distribution_root = Path(__file__).resolve().parents[4]
    if (distribution_root / "hints" / "index.json").is_file():
        return distribution_root
    die("[hint_tag] FAIL: Git checkout 또는 self-contained distribution root를 찾을 수 없습니다.")
    raise RuntimeError("unreachable after die()")


def require_git_repository() -> None:
    try:
        out = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--is-inside-work-tree"],
                             capture_output=True, text=True)
    except OSError:
        die("[hint_tag] FAIL: match 이외 명령은 Git 실행파일과 실제 Git 레포가 필요합니다.")
        return
    if out.returncode != 0 or out.stdout.strip() != "true":
        die("[hint_tag] FAIL: match 이외 명령은 Git 레포 안에서만 실행할 수 있습니다.")


ROOT = repo_root()
PII_TERMS_FILE = ROOT / ".claude" / "pii_terms.txt"
INDEX_FILE = ROOT / "hints" / "index.json"
# hint 카탈로그(부록 표)의 홈 = 전용 HINTS.md(README 는 링크 참조만 — 21+ 행이 README 를
# 비대하게 만들던 문제 교정, plan_26070222 Token Economy 의 문서 축 연장).
HINTS_FILE = ROOT / "HINTS.md"
TEMPLATE_FILE = ROOT / ".claude" / "skills" / "upstream-version-watch" / "templates" / "hint_recipe.template.md"
DRAFTS_DIR = ROOT / "hints" / ".drafts"


def git(*args: str, check: bool = True, env: dict | None = None, input_text: str | None = None) -> subprocess.CompletedProcess:
    out = subprocess.run(["git", *args], capture_output=True, text=True, cwd=str(ROOT), env=env,
                          input=input_text)
    if check and out.returncode != 0:
        die(f"[hint_tag] git {' '.join(args)} FAILED:\n{out.stderr.strip()}")
    return out


# ── promotion-gate wiring (Phase 3, plan_26072506, vertical slice 2A) ────────
# Every subcommand that mutates a git tag/hints/index.json/HINTS.md, or performs a network push,
# requires an explicit --manifest and must pass `completion_gate.py authorize --mode promotion`
# for its mapped action BEFORE any command-specific validation that could itself have a side
# effect. `match` is read-only (near-miss scoring only) and stays deliberately ungated.
COMPLETION_GATE_SCRIPT = ROOT / ".claude" / "policies" / "runtime" / "completion_gate.py"

HINT_ACTION_FOR_CMD: dict[str, str] = {
    "create": "hint_create",
    "finalize": "hint_finalize",
    "verify": "hint_verify",
    "reindex": "hint_reindex",
    "push": "hint_push",
    "reverify": "hint_reverify",
}


def _gate_bare_result(action: str, code: str, message: str) -> dict:
    return {
        "schema_version": 1, "mode": "promotion", "action": action, "task_class": None,
        "authorization_state": None, "allowed": False,
        "reason_codes": [code], "messages": {code: message}, "identity": None, "exit_code": 2,
    }


def _require_promotion_authorization(cmd: str, manifest_path: str) -> None:
    """Synchronous promotion-mode gate check -- the FIRST action every gated hint_tag.py
    subcommand takes, before any git tag/index/draft/HINTS mutation or network push. A denial (or
    a gate subprocess that could not be run/parsed at all) prints a stable JSON result to stdout
    (the real completion_gate.py stdout verbatim when it produced one, else a deterministic
    wrapper in the same shape) and exits non-zero -- no hint_tag.py side effect has happened yet
    at the point every gated cmd_* function calls this, so denial/failure here always means zero
    side effects."""
    action = HINT_ACTION_FOR_CMD[cmd]
    manifest_arg = Path(manifest_path)
    resolved_manifest = manifest_arg if manifest_arg.is_absolute() else (Path.cwd() / manifest_arg)

    try:
        proc = subprocess.run(
            [sys.executable, str(COMPLETION_GATE_SCRIPT), "authorize",
             "--manifest", str(resolved_manifest), "--mode", "promotion",
             "--action", action, "--repo-root", str(ROOT)],
            capture_output=True, text=True, timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as e:
        print(json.dumps(_gate_bare_result(action, "HINT_GATE_SUBPROCESS_UNAVAILABLE",
                                            f"could not execute completion_gate.py authorize: {e}"),
                          ensure_ascii=False, sort_keys=True, indent=2))
        raise SystemExit(2)

    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError:
        print(json.dumps(_gate_bare_result(
            action, "HINT_GATE_OUTPUT_UNREADABLE",
            f"completion_gate.py authorize (exit={proc.returncode}) did not produce parseable "
            f"JSON stdout; stderr={proc.stderr.strip()[:500]!r}"),
            ensure_ascii=False, sort_keys=True, indent=2))
        raise SystemExit(2)

    if proc.returncode == 0 and parsed.get("allowed") is True:
        return  # authorized -- proceed to the command's own logic, no output printed here.

    stdout = proc.stdout
    sys.stdout.write(stdout if stdout.endswith("\n") else stdout + "\n")
    raise SystemExit(proc.returncode if proc.returncode != 0 else 1)


# ── hint-evidence-binding (Phase 3 review-remediation, subagent-summary-0-20260725_160544_088157)
# ────────────────────────────────────────────────────────────────────────────
# Closes 3 blocking findings: (1) promotion evidence must be BOUND to the specific hint being
# authorized -- a generic promotion-ready manifest, or one bound to an unrelated tag/topology/
# anchor/identity, must never authorize create/finalize/reverify for a DIFFERENT hint; (2) verify/
# reindex/push must fail closed for hint tags lacking a durable evidence-binding footer (unbound
# historical tags), never silently accepting an unrelated manifest as if it vouched for the whole
# tag set; (3) create's emitted follow-up finalize command must stay literally executable.
#
# The durable binding lives in the annotated tag's OWN body (never the source recipe file, which
# finalize never mutates) as a strict, flat `key: value` footer block bracketed by HTML-comment
# markers (invisible when the recipe is rendered, always present in the raw tag object text that
# PII-scanning/parsing already operate on):
#
#   <!-- hint-evidence-binding:v1
#   version: 1
#   tag: hint/<vllm>/<model>/<arch>
#   topology: <exact --topology string the hint command was invoked with>
#   anchor: <full 40-hex commit SHA>
#   manifest_sha256: <sha256 of the EXACT --manifest file bytes, at finalize time>
#   identity_sha256: <sha256 of the canonical (sorted, compact-JSON) 6-key strong identity>
#   manifest_ref: <normalized repo-relative manifest path; re-opened and revalidated by every global command>
#   certificate_sha256: <sha256 of the certificate artifact file bytes, at finalize time>
#   certificate_ref: <manifest-relative certificate path; re-opened and digest-checked>
#   -->
#
# All 9 fields are required whenever the block is present at all -- no duplicates, no unrecognized
# keys, no malformed anchor/digest formats. Missing the block entirely (a historical/unmigrated
# tag) and a present-but-malformed block are two DISTINCT, stable reason codes.

_FOOTER_MARKER_OPEN = "<!-- hint-evidence-binding:v1"
_FOOTER_MARKER_CLOSE = "-->"
_FOOTER_FIELDS = (
    "version", "tag", "topology", "anchor", "manifest_sha256", "identity_sha256",
    "manifest_ref", "certificate_sha256", "certificate_ref",
)
_FOOTER_BLOCK_RE = re.compile(
    re.escape(_FOOTER_MARKER_OPEN) + r"\s*\n(?P<body>.*?)\n" + re.escape(_FOOTER_MARKER_CLOSE),
    re.S,
)
_FOOTER_LINE_RE = re.compile(r"^([a-z0-9_]+): (.*)$")
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_FULL_ANCHOR_RE = re.compile(r"^[0-9a-f]{40}$")


class HintEvidenceBindingError(Exception):
    """Carries a stable (code, message) pair for a missing/malformed evidence-binding footer --
    the two callers (verify's per-tag problem list, reindex/push's atomic pre-write gate) each
    render it their own way rather than one parse path being hardwired to one output shape."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


def _build_evidence_footer(fields: dict[str, str]) -> str:
    lines = [_FOOTER_MARKER_OPEN]
    lines.extend(f"{k}: {fields[k]}" for k in _FOOTER_FIELDS)
    lines.append(_FOOTER_MARKER_CLOSE)
    return "\n".join(lines) + "\n"


def _parse_evidence_footer(body: str) -> dict[str, str]:
    """Strict parse of ONE hint-evidence-binding footer out of a tag object's body text. Raises
    HintEvidenceBindingError -- HINT_EVIDENCE_BINDING_MISSING when the block itself isn't found at
    all (a historical/unmigrated tag), HINT_EVIDENCE_BINDING_MALFORMED for every other defect
    (missing/duplicate/unrecognized field, bad version, or a value that fails its own format
    check) -- never silently accepts a partially-formed block."""
    m = _FOOTER_BLOCK_RE.search(body)
    if not m:
        raise HintEvidenceBindingError(
            "HINT_EVIDENCE_BINDING_MISSING",
            "no hint-evidence-binding footer found in the tag object body",
        )
    fields: dict[str, str] = {}
    for raw_line in m.group("body").splitlines():
        line = raw_line.strip("\r")
        if not line.strip():
            continue
        fm = _FOOTER_LINE_RE.match(line)
        if not fm:
            raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                            f"unparseable footer line: {line!r}")
        key, value = fm.group(1), fm.group(2).strip()
        if key not in _FOOTER_FIELDS:
            raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                            f"unrecognized footer key: {key!r}")
        if key in fields:
            raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                            f"duplicate footer key: {key!r}")
        fields[key] = value
    missing = [k for k in _FOOTER_FIELDS if k not in fields]
    if missing:
        raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                        f"footer missing required field(s): {missing}")
    if fields["version"] != "1":
        raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                        f"unsupported footer version: {fields['version']!r}")
    if not _FULL_ANCHOR_RE.match(fields["anchor"]):
        raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                        f"anchor is not a full 40-hex commit SHA: {fields['anchor']!r}")
    for k in ("manifest_sha256", "identity_sha256", "certificate_sha256"):
        if not _SHA256_RE.match(fields[k]):
            raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                            f"{k} is not a 64-hex sha256 digest: {fields[k]!r}")
    for k in ("tag", "topology", "manifest_ref", "certificate_ref"):
        if not fields[k]:
            raise HintEvidenceBindingError("HINT_EVIDENCE_BINDING_MALFORMED",
                                            f"footer field {k!r} is empty")
    return fields


def _tag_object_body(name: str) -> str:
    _, _, body = git("cat-file", "tag", name).stdout.partition("\n\n")
    return body


def _die_binding(action: str, reason_codes: list[str], messages: dict[str, str],
                  identity: dict | None, task_class: str | None, exit_code: int = 2) -> "NoReturn":
    """Stable, machine-readable rejection -- same envelope shape as _require_promotion_authorization
    (side-effect-authorization-like), for every hint-specific binding failure this module raises on
    top of (never inside) the shared completion_gate.py state machine. Printed BEFORE any
    draft/tag/index/HINTS/push mutation the calling cmd_* function would otherwise perform."""
    result = {
        "schema_version": 1, "mode": "promotion", "action": action, "task_class": task_class,
        "authorization_state": None, "allowed": False,
        "reason_codes": sorted(reason_codes), "messages": messages,
        "identity": identity, "exit_code": exit_code,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    raise SystemExit(exit_code)


def _load_manifest_for_binding(action: str, manifest_path_str: str) -> tuple[dict, Path]:
    """Safely (re-)loads the exact manifest file hint_tag.py's own binding logic needs to inspect
    (promotion_target/identity/evidence.certificate) -- resolved the same way
    _require_promotion_authorization resolves it (relative to CWD unless absolute). By the time
    this runs, the common gate has already proven the file parses as schema-valid JSON, so failure
    here is defensive (should not normally trigger), not the primary validation path."""
    manifest_arg = Path(manifest_path_str)
    resolved = manifest_arg if manifest_arg.is_absolute() else (Path.cwd() / manifest_arg)
    try:
        raw = resolved.read_bytes()
    except OSError as e:
        _die_binding(action, ["HINT_MANIFEST_UNREADABLE"],
                     {"HINT_MANIFEST_UNREADABLE": f"could not read manifest {resolved}: {e}"}, None, None)
    try:
        manifest = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as e:
        _die_binding(action, ["HINT_MANIFEST_INVALID_JSON"],
                     {"HINT_MANIFEST_INVALID_JSON": f"manifest {resolved} is not valid JSON: {e}"}, None, None)
    if not isinstance(manifest, dict):
        _die_binding(action, ["HINT_MANIFEST_INVALID_SHAPE"],
                     {"HINT_MANIFEST_INVALID_SHAPE": f"manifest {resolved} root is not a JSON object"}, None, None)
    return manifest, resolved


_cgate_module = None


def _cgate():
    """Lazy same-directory import of completion_gate.py (Python puts a directly-run script's own
    directory on sys.path[0], so this works identically in an isolated test repo where
    completion_gate.py is copied into the same scripts/ directory). Reuses its already-hardened,
    symlink-rejecting evidence path resolver and STRONG_IDENTITY_FIELDS constant instead of a
    second, hand-duplicated path-safety implementation here (pointer principle). Deliberately NOT
    a module-level import: `match` never needs it and must keep working even when
    completion_gate.py is entirely absent (it stays read-only/ungated by design)."""
    global _cgate_module
    if _cgate_module is None:
        import completion_gate as _cg
        _cgate_module = _cg
    return _cgate_module


def _canonical_identity_sha256(identity: dict) -> str:
    """Deterministic canonical serialization of the 6-key strong identity (single source of the
    field set/order: completion_gate.py's own STRONG_IDENTITY_FIELDS), hashed for the footer/
    verify recompute-compare -- compact separators avoid whitespace ambiguity."""
    payload = {f: identity.get(f) for f in _cgate().STRONG_IDENTITY_FIELDS}
    blob = json.dumps(payload, sort_keys=True, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()


def _topology_class(topology: str) -> str:
    return "multi" if topology.strip().lower().startswith("multi") else "single"


_TP_LABEL_RE = re.compile(r"\bTP(\d+)\b", re.IGNORECASE)


def _require_hint_promotion_target(action: str, manifest: dict, *, tag: str, topology: str,
                                    anchor: str, vllm: str, model: str) -> dict:
    """Cross-checks manifest['promotion_target'] against the CLI-derived tag/topology/anchor this
    command is actually acting on, and manifest['identity'] against the tag's <vllm>/<model>
    segments + derived topology class + an unambiguous TP<n> topology-label crosscheck (TP itself
    stays certificate-bound through the common completion gate). Dies (stable JSON, zero mutation
    -- called before any draft/tag/index write) on the first class of mismatch found; returns
    manifest['promotion_target'] on success. A generic promotion-ready manifest -- or one bound to
    a DIFFERENT hint -- is never sufficient (blocker 1)."""
    task_class = manifest.get("task_class") if isinstance(manifest, dict) else None
    identity = manifest.get("identity") if isinstance(manifest, dict) else None
    pt = manifest.get("promotion_target") if isinstance(manifest, dict) else None

    if not isinstance(pt, dict):
        _die_binding(action, ["HINT_PROMOTION_TARGET_MISSING"],
                     {"HINT_PROMOTION_TARGET_MISSING":
                      "manifest has no promotion_target block -- hint_* actions require an exact "
                      "hint-specific promotion binding, a generic promotion-ready manifest is not "
                      "enough"},
                     identity, task_class)

    target_mismatches: list[tuple[str, str]] = []
    if pt.get("tag") != tag:
        target_mismatches.append(("HINT_PROMOTION_TARGET_TAG_MISMATCH",
                                   f"promotion_target.tag={pt.get('tag')!r} != requested tag {tag!r}"))
    if pt.get("topology") != topology:
        target_mismatches.append(("HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH",
                                   f"promotion_target.topology={pt.get('topology')!r} != requested "
                                   f"topology {topology!r}"))
    if pt.get("anchor") != anchor:
        target_mismatches.append(("HINT_PROMOTION_TARGET_ANCHOR_MISMATCH",
                                   f"promotion_target.anchor={pt.get('anchor')!r} != resolved anchor {anchor!r}"))
    if target_mismatches:
        _die_binding(action, [c for c, _ in target_mismatches], {c: m for c, m in target_mismatches},
                     identity, task_class)

    id_mismatches: list[tuple[str, str]] = []
    if identity.get("model") != model:
        id_mismatches.append(("HINT_IDENTITY_MODEL_MISMATCH",
                               f"identity.model={identity.get('model')!r} != tag model {model!r}"))
    m_vllm = identity.get("vllm")
    if not (m_vllm == vllm or (isinstance(m_vllm, str) and m_vllm.startswith(vllm + "-"))):
        id_mismatches.append(("HINT_IDENTITY_VLLM_MISMATCH",
                               f"identity.vllm={m_vllm!r} does not bind to tag vllm {vllm!r} (must "
                               f"equal it, or begin with {vllm + '-'!r} for a fork suffix)"))
    expected_class = _topology_class(topology)
    if identity.get("topology") != expected_class:
        id_mismatches.append(("HINT_IDENTITY_TOPOLOGY_CLASS_MISMATCH",
                               f"identity.topology={identity.get('topology')!r} != topology class "
                               f"{expected_class!r} derived from {topology!r}"))
    tp_label = _TP_LABEL_RE.search(topology)
    if tp_label is not None and identity.get("tp") != int(tp_label.group(1)):
        id_mismatches.append(("HINT_IDENTITY_TP_MISMATCH",
                               f"identity.tp={identity.get('tp')!r} != TP{tp_label.group(1)} in "
                               f"topology label {topology!r}"))
    if id_mismatches:
        _die_binding(action, [c for c, _ in id_mismatches], {c: m for c, m in id_mismatches},
                     identity, task_class)

    return pt


def _resolve_evidence_footer_fields(action: str, manifest: dict, resolved_manifest_path: Path,
                                     manifest_arg_str: str, *, tag: str, topology: str, anchor: str) -> dict[str, str]:
    """Independently (re-)resolves every value the durable evidence-binding footer records.
    NEVER trusts the common completion_gate.py gate's prior approval as a substitute for
    hint_tag.py's own read here -- an unsafe/missing manifest/certificate is rejected despite the
    common gate having already passed the same manifest (blocker-1/2 remediation).

    manifest_ref MUST be a normalized, safely-resolvable REPO-RELATIVE regular file -- there is no
    'declared string, diagnostics only' fallback for a manifest outside ROOT: every OTHER hint
    command validates EVERY tag purely from ROOT + that tag's own footer, years after the original
    --manifest CLI argument that created it is gone, so a reference that cannot be durably
    re-resolved from ROOT alone is not a valid binding at all."""
    identity = manifest.get("identity") or {}
    identity_sha256 = _canonical_identity_sha256(identity)

    # Pure lexical normalization (no I/O, no symlink following) -- mirrors completion_gate.py's own
    # _lexical_components: dividing an absolute path onto anything on the left discards the left
    # operand, so this correctly normalizes `resolved_manifest_path` (always absolute) regardless
    # of what it's joined against.
    lexical_abs = Path(os.path.normpath(str(resolved_manifest_path)))
    try:
        manifest_ref = str(lexical_abs.relative_to(ROOT))
    except ValueError:
        _die_binding(action, ["HINT_MANIFEST_REF_OUTSIDE_REPO"],
                     {"HINT_MANIFEST_REF_OUTSIDE_REPO":
                      f"--manifest {manifest_arg_str!r} resolves outside the repo root -- a durable "
                      f"evidence-binding footer requires a repo-relative manifest reference"},
                     identity, manifest.get("task_class"))

    repo_root_fd = os.open(str(ROOT), os.O_RDONLY | os.O_DIRECTORY)
    try:
        r_manifest = _cgate().resolve_and_stat_evidence(repo_root_fd, ROOT, ROOT, manifest_ref, expect_dir=False)
        if r_manifest["status"] != "ok":
            _die_binding(action, ["HINT_MANIFEST_REF_UNSAFE"],
                         {"HINT_MANIFEST_REF_UNSAFE":
                          f"manifest_ref {manifest_ref!r} could not be safely resolved+hashed "
                          f"(status={r_manifest['status']}) -- refusing to bind an unsafe/symlinked "
                          f"manifest reference into the durable footer"},
                         identity, manifest.get("task_class"))
        manifest_sha256 = r_manifest["sha256"]

        cert_item = (manifest.get("evidence") or {}).get("certificate")
        cert_path_str = cert_item.get("path") if isinstance(cert_item, dict) else None
        if not cert_path_str:
            _die_binding(action, ["HINT_CERTIFICATE_EVIDENCE_MISSING"],
                         {"HINT_CERTIFICATE_EVIDENCE_MISSING":
                          "manifest has no evidence.certificate.path -- cannot bind a durable "
                          "evidence footer without a certificate artifact to hash"},
                         identity, manifest.get("task_class"))
        r_cert = _cgate().resolve_and_stat_evidence(repo_root_fd, ROOT, resolved_manifest_path.parent,
                                                     cert_path_str, expect_dir=False)
        if r_cert["status"] != "ok":
            _die_binding(action, ["HINT_CERTIFICATE_UNSAFE_OR_MISSING"],
                         {"HINT_CERTIFICATE_UNSAFE_OR_MISSING":
                          f"certificate {cert_path_str!r} could not be safely resolved+hashed "
                          f"(status={r_cert['status']}) -- refusing despite the common gate's own approval"},
                         identity, manifest.get("task_class"))
    finally:
        os.close(repo_root_fd)

    return {
        "version": "1", "tag": tag, "topology": topology, "anchor": anchor,
        "manifest_sha256": manifest_sha256, "identity_sha256": identity_sha256,
        "manifest_ref": manifest_ref, "certificate_sha256": r_cert["sha256"], "certificate_ref": cert_path_str,
    }


def load_pii_terms() -> list[str] | None:
    """Same source as scan_forbidden_strings.py (pointer principle). None = absent."""
    if not PII_TERMS_FILE.is_file():
        return None
    terms = []
    for line in PII_TERMS_FILE.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            terms.append(line)
    return terms


def scan_text(text: str, terms: list[str] | None, skip_generic: frozenset[str] = frozenset()) -> list[str]:
    """Scan for pii_terms literals + generic patterns. `skip_generic` drops named generic
    patterns — the tagger identity check skips 'email' (a tagger MUST have an email; we only
    forbid it carrying a KNOWN-PII literal like a personal handle/domain, not being an email)."""
    hits = []
    for t in terms or []:
        if t and t in text:
            hits.append(f"term:{t}")
    for name, pat in GENERIC_PII:
        if name in skip_generic:
            continue
        m = pat.search(text)
        if m:
            hits.append(f"{name}:{m.group(0)}")
    return hits


def canonicalize(model: str) -> str | None:
    for canon, spellings in CANONICAL_SLUGS.items():
        if model == canon or model in spellings:
            return canon
    return None


def existing_hint_tags() -> list[str]:
    return [t for t in git("tag", "-l", "hint/*").stdout.split() if t]


def validate_name(name: str, allow_new_slug: bool = False, expect_absent: bool = True) -> tuple[str, str, str]:
    if not TAG_SHAPE.match(name):
        die(f"[hint_tag] FAIL: 이름 형태 위반(hint/<vllm>/<model>/<arch>): {name}")
    if git("check-ref-format", f"refs/tags/{name}", check=False).returncode != 0:
        die(f"[hint_tag] FAIL: git 이 거부하는 ref 이름: {name}")
    _, vllm, model, arch = name.split("/")
    canon = canonicalize(model)
    if canon is None and not allow_new_slug:
        die(f"[hint_tag] FAIL: 모델 슬러그 '{model}' 가 정본표에 없음. "
            f"CANONICAL_SLUGS 에 등록하거나 --allow-new-slug(HITL) 사용.")
    if canon is not None and canon != model:
        die(f"[hint_tag] FAIL: 별칭 '{model}' 대신 정본 슬러그 '{canon}' 를 쓰세요.")
    existing = existing_hint_tags()
    if expect_absent and name in existing:
        die(f"[hint_tag] FAIL: 이미 존재하는 태그: {name}")
    for e in existing:
        if e == name:
            continue
        if e.startswith(name + "/") or name.startswith(e + "/"):
            die(f"[hint_tag] FAIL: 기존 태그와 D/F prefix 충돌: {e}")
    return vllm, model, arch


def _load_index() -> dict:
    return json.loads(INDEX_FILE.read_text(encoding="utf-8"))


def _save_index(idx: dict) -> None:
    INDEX_FILE.write_text(json.dumps(idx, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _brief_of(body: str) -> str:
    for line in body.splitlines():
        if line.strip():
            return line.strip()  # 첫 비어있지 않은 줄 = brief (템플릿 첫 줄 = {{BRIEF}})
    return ""


def _parse_tag_body(name: str) -> tuple[str, str, str]:
    """태그 오브젝트 본문에서 (brief, topology, related) 파싱 — reindex 용."""
    _, _, body = git("cat-file", "tag", name).stdout.partition("\n\n")
    mt = re.search(r"topology:(.+?) · date:", body)
    mr = re.search(r"related:\s*(.+)", body)
    return _brief_of(body), (mt.group(1).strip() if mt else ""), (mr.group(1).strip() if mr else "")


# ── create ──────────────────────────────────────────────────────────────────
def cmd_create(a: argparse.Namespace) -> int:
    _require_promotion_authorization("create", a.manifest)
    vllm, model, arch = validate_name(a.tag, a.allow_new_slug)
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    manifest, _ = _load_manifest_for_binding("hint_create", a.manifest)
    _require_hint_promotion_target("hint_create", manifest, tag=a.tag, topology=a.topology,
                                    anchor=anchor, vllm=vllm, model=model)

    rj: dict = {}
    rp = (ROOT / a.from_resolved)
    if rp.is_file():
        try:
            rj = json.loads(rp.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            rj = {}

    def scalar(path: list[str], default: str = "<채워넣기>") -> str:
        cur = rj
        for k in path:
            if not isinstance(cur, dict):
                return default
            cur = cur.get(k)
            if cur is None:
                return default
        return cur if isinstance(cur, str) else default

    dg_branch = scalar(["build_lib_pins", "deepgemm", "branch"], "")
    subs = {
        "VLLM": vllm, "MODEL": model, "ARCH": arch, "BRIEF": a.brief or "<한줄 요약>",
        "TOPOLOGY": a.topology, "DATE": date.today().isoformat(),
        "CUDA": scalar(["ngc_base", "cuda_version"]),
        "TORCH_PIN": scalar(["torch", "pin"]),
        "NGC_TAG": scalar(["ngc_base", "tag"]),
        "BUILD_TRACK": scalar(["build_track", "decision"]),
        "CPU_ARCH": scalar(["cpu_arch"]),
        "DEEPGEMM_REF": scalar(["build_lib_pins", "deepgemm", "ref"])[:12],
        "DEEPGEMM_BRANCH": (dg_branch.split()[0] if dg_branch else "<채워넣기>"),
        "TAG": a.tag, "ANCHOR_SHA": anchor[:12],
        "TOPO_DIR": "multi" if a.topology.strip().startswith("multi") else "single",
        "RELATED": a.related or "-",
    }
    tpl = TEMPLATE_FILE.read_text(encoding="utf-8")
    for k, v in subs.items():
        tpl = tpl.replace("{{" + k + "}}", str(v))

    DRAFTS_DIR.mkdir(parents=True, exist_ok=True)
    out = DRAFTS_DIR / (a.tag.replace("/", "_") + ".md")
    out.write_text(tpl, encoding="utf-8")
    follow_up = ["python3", ".claude/skills/upstream-version-watch/scripts/hint_tag.py", "finalize",
                 "--tag", a.tag, "--recipe", str(out.relative_to(ROOT)),
                 "--commit", anchor[:12], "--topology", a.topology,
                 "--manifest", a.manifest]
    if a.related:
        follow_up += ["--related", a.related]
    if a.allow_new_slug:
        follow_up.append("--allow-new-slug")
    print(f"[hint_tag] scaffold → {out.relative_to(ROOT)}  (앵커 {anchor[:12]})")
    print("[hint_tag] 다음: TODO(judgment) 슬롯을 채운 뒤:")
    print("           " + " ".join(shlex.quote(tok) for tok in follow_up))
    return 0


# ── finalize ────────────────────────────────────────────────────────────────
RE_ABS_HOST = re.compile(r"\b\d{2,}\s?GiB\b|\b\d{9,}\b|gmu\s*0?\.\d")
RE_REMEASURE = re.compile(r"측정|재도출|재측정|비이식|re-?measure|measure")


def _assert_remeasure(body: str) -> None:
    if RE_ABS_HOST.search(body) and not RE_REMEASURE.search(body):
        die("[hint_tag] FAIL(B1 백스톱): 절대 호스트-스코프 숫자(KV GiB/bytes/gmu)가 있으나 "
            "재측정 한정자(측정/재도출/비이식)가 없음 — 슬롯4를 measurement-first 로 재작성.")


def cmd_finalize(a: argparse.Namespace) -> int:
    _require_promotion_authorization("finalize", a.manifest)
    vllm, model, arch = validate_name(a.tag, a.allow_new_slug)  # 미존재·정본·합법 재확인
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    manifest, resolved_manifest_path = _load_manifest_for_binding("hint_finalize", a.manifest)
    _require_hint_promotion_target("hint_finalize", manifest, tag=a.tag, topology=a.topology,
                                    anchor=anchor, vllm=vllm, model=model)
    footer_fields = _resolve_evidence_footer_fields("hint_finalize", manifest, resolved_manifest_path,
                                                     a.manifest, tag=a.tag, topology=a.topology, anchor=anchor)

    recipe = Path(a.recipe) if os.path.isabs(a.recipe) else (ROOT / a.recipe)
    if not recipe.is_file():
        die(f"[hint_tag] FAIL: 레시피 파일 없음: {a.recipe}")
    body = recipe.read_text(encoding="utf-8")

    if "TODO(judgment" in body:
        die("[hint_tag] FAIL: 레시피에 TODO(judgment) 슬롯이 남아있음 — 에이전트가 저작해야 함.")

    terms = load_pii_terms()
    if terms is None:
        die("[hint_tag] FAIL(fail-closed): .claude/pii_terms.txt 부재 — PII-clean 인증 불가.")

    hits = scan_text(body, terms)
    if hits:
        die("[hint_tag] FAIL(PII): 레시피 본문 PII 검출:\n  " + "\n  ".join(hits))

    _assert_remeasure(body)

    footer_text = _build_evidence_footer(footer_fields)
    footer_hits = scan_text(footer_text, terms)
    if footer_hits:
        die("[hint_tag] FAIL(PII): evidence-binding footer PII 검출:\n  " + "\n  ".join(footer_hits))

    tname = a.tagger_name or git("config", "user.name", check=False).stdout.strip()
    temail = a.tagger_email or git("config", "user.email", check=False).stdout.strip()
    idhits = scan_text(f"{tname} {temail}", terms, skip_generic=frozenset({"email"}))
    if idhits:
        die("[hint_tag] FAIL(tagger PII): tagger 신원이 PII 를 흘림: " + ", ".join(idhits)
            + "\n  → --tagger-name/--tagger-email 로 clean 한 공개 신원을 지정하세요"
            " (배포 태그 오브젝트에 tagger 가 박힙니다).")

    # The tag message = original recipe body + footer, streamed via stdin (`-F -`) -- the source
    # recipe FILE on disk is never rewritten (design requirement: finalize must not mutate it).
    env = dict(os.environ, GIT_COMMITTER_NAME=tname, GIT_COMMITTER_EMAIL=temail)
    tag_message = (body if body.endswith("\n") else body + "\n") + "\n" + footer_text
    git("-c", f"user.name={tname}", "-c", f"user.email={temail}",
        "tag", "-a", a.tag, anchor, "-F", "-", env=env, input_text=tag_message)
    print(f"[hint_tag] tagged {a.tag} → {anchor[:12]}  (tagger {tname} <{temail}>)")

    brief = _brief_of(body)
    idx = _load_index()
    idx["hints"] = [e for e in idx["hints"] if e["tag"] != a.tag]
    idx["hints"].append({
        "tag": a.tag, "vllm": vllm, "model": model, "arch": arch,
        "topology": a.topology, "brief": brief, "anchor": anchor,
        "related": a.related or "", "status": "active",
        "last_verified": date.today().isoformat(),
    })
    idx["hints"].sort(key=lambda e: e["tag"])
    _save_index(idx)
    _hints_regen(idx["hints"])
    print("[hint_tag] index.json + HINTS.md 카탈로그 인덱스 갱신 완료.")
    return 0


def _hints_row(e: dict) -> str:
    return (f"| `{e['tag']}` | {e['vllm']} | {e['model']} | {e['arch']} | "
            f"{e.get('topology','')} | {e.get('status','active')} | "
            f"{e.get('superseded_by') or e.get('related') or '—'} | "
            f"{e.get('last_verified','')} | {e.get('brief','')} |")


def _hints_regen(hints: list[dict]) -> None:
    """HINTS.md 카탈로그 행 전량 재생성(index = 진실원천). 마커 앞에 정렬 삽입."""
    if not HINTS_FILE.is_file():
        return
    out = []
    for ln in HINTS_FILE.read_text(encoding="utf-8").splitlines():
        if ln.startswith("| `hint/"):
            continue  # 기존 hint 행 전부 제거
        if ln.strip() == HINTS_MARKER:
            out.extend(_hints_row(e) for e in hints)
        out.append(ln)
    HINTS_FILE.write_text("\n".join(out) + "\n", encoding="utf-8")


# ── verify ──────────────────────────────────────────────────────────────────
def _evaluate_manifest_promotion_ready(manifest_path: Path, action: str) -> tuple[bool, str]:
    """Read-only re-evaluation of ONE referenced manifest through the REAL completion_gate.py
    promotion gate (subprocess -- never re-derives/duplicates its heavily-reviewed state-machine
    logic), for the CURRENT hint action. `authorize` has no side effects of its own beyond this
    read-only subprocess. Returns (ok, detail) -- detail is a short, stable summary (reason codes
    only), never the full nested JSON the subprocess printed (kept OUT of problem messages on
    purpose -- a per-tag problem line must stay one line of stable text, not an embedded second
    JSON document)."""
    try:
        proc = subprocess.run(
            [sys.executable, str(COMPLETION_GATE_SCRIPT), "authorize",
             "--manifest", str(manifest_path), "--mode", "promotion",
             "--action", action, "--repo-root", str(ROOT)],
            capture_output=True, text=True, timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as e:
        return False, f"gate subprocess unavailable: {e}"
    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return False, f"gate subprocess output unreadable (exit={proc.returncode})"
    if proc.returncode == 0 and parsed.get("allowed") is True:
        return True, ""
    return False, f"exit={proc.returncode} reason_codes={parsed.get('reason_codes')}"


def _validate_hint_tag_evidence(tag: str, action: str) -> tuple[dict[str, str] | None, list[str]]:
    """Full evidence-binding validation for ONE existing hint tag, driven ENTIRELY from that tag's
    own footer + ROOT -- NEVER from whatever --manifest the CURRENTLY-RUNNING command happened to
    receive. Closes the fabricated-historical-footer gap (cycle-2 remediation): a syntactically
    valid footer whose manifest_ref/certificate_ref don't actually exist/resolve safely, whose
    declared digests don't match the real bytes, whose referenced manifest doesn't itself vouch
    (promotion-ready + its OWN promotion_target/identity binding) for this exact tag, or whose
    footer.anchor disagrees with the tag's actual peeled commit, all fail here -- regardless of
    what OTHER manifest the running command was invoked with.

    Returns (footer, problems). `footer` is None only when the footer itself could not be parsed
    at all (missing/malformed) -- every other failure still returns the parsed footer (so a caller
    like reindex that needs topology/anchor for reconstruction only requires the footer to be
    well-formed; any problem still blocks the write via the caller's own atomicity check). An empty
    `problems` list means this tag's evidence is genuinely durable."""
    typ = git("cat-file", "-t", tag, check=False).stdout.strip()
    if typ != "tag":
        return None, [f"{tag}: HINT_TAG_NOT_ANNOTATED annotated 태그가 아님(type={typ})"]
    try:
        footer = _parse_evidence_footer(_tag_object_body(tag))
    except HintEvidenceBindingError as e:
        return None, [f"{tag}: {e.code} {e.message}"]

    problems: list[str] = []

    if footer["tag"] != tag:
        problems.append(f"{tag}: HINT_EVIDENCE_BINDING_TAG_SELF_MISMATCH footer.tag={footer['tag']!r} != actual tag {tag!r}")

    actual_commit = git("rev-parse", "--verify", tag + "^{commit}", check=False).stdout.strip()
    if not actual_commit:
        problems.append(f"{tag}: HINT_TAG_COMMIT_UNRESOLVABLE 태그의 peeled commit 을 확인할 수 없음")
    elif footer["anchor"] != actual_commit:
        problems.append(f"{tag}: HINT_EVIDENCE_BINDING_ANCHOR_SELF_MISMATCH footer.anchor={footer['anchor']!r} "
                         f"!= actual peeled commit {actual_commit!r}")

    repo_root_fd = os.open(str(ROOT), os.O_RDONLY | os.O_DIRECTORY)
    try:
        r_manifest = _cgate().resolve_and_stat_evidence(repo_root_fd, ROOT, ROOT, footer["manifest_ref"],
                                                         expect_dir=False, capture_content=True)
    finally:
        os.close(repo_root_fd)
    if r_manifest["status"] != "ok":
        problems.append(f"{tag}: HINT_EVIDENCE_MANIFEST_REF_UNSAFE_OR_MISSING manifest_ref "
                         f"{footer['manifest_ref']!r} resolve 실패(status={r_manifest['status']})")
        return footer, problems  # nothing further can be checked without the manifest bytes
    if r_manifest["sha256"] != footer["manifest_sha256"]:
        problems.append(f"{tag}: HINT_EVIDENCE_MANIFEST_SHA_MISMATCH {r_manifest['sha256']} != {footer['manifest_sha256']}")

    try:
        ref_manifest = json.loads((r_manifest["content_bytes"] or b"").decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        problems.append(f"{tag}: HINT_EVIDENCE_MANIFEST_INVALID_JSON manifest_ref {footer['manifest_ref']!r} "
                         f"가 유효한 JSON 이 아님")
        return footer, problems
    if not isinstance(ref_manifest, dict):
        problems.append(f"{tag}: HINT_EVIDENCE_MANIFEST_INVALID_SHAPE manifest_ref root 가 JSON object 아님")
        return footer, problems

    manifest_ref_abs = ROOT / footer["manifest_ref"]  # already proven safe/regular above
    gate_ok, gate_detail = _evaluate_manifest_promotion_ready(manifest_ref_abs, action)
    if not gate_ok:
        problems.append(f"{tag}: HINT_EVIDENCE_MANIFEST_NOT_PROMOTION_READY {gate_detail}")

    _, vllm, model, arch = tag.split("/")
    ref_pt = ref_manifest.get("promotion_target")
    if not isinstance(ref_pt, dict):
        problems.append(f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_MISSING referenced manifest 에 promotion_target 없음")
    else:
        if ref_pt.get("kind") != "hint":
            problems.append(f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_KIND_MISMATCH kind={ref_pt.get('kind')!r}")
        if ref_pt.get("tag") != tag:
            problems.append(f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_TAG_MISMATCH "
                             f"promotion_target.tag={ref_pt.get('tag')!r} != {tag!r}")
        if ref_pt.get("topology") != footer["topology"]:
            problems.append(f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_TOPOLOGY_MISMATCH "
                             f"promotion_target.topology={ref_pt.get('topology')!r} != footer topology {footer['topology']!r}")
        if ref_pt.get("anchor") != footer["anchor"]:
            problems.append(f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_ANCHOR_MISMATCH "
                             f"promotion_target.anchor={ref_pt.get('anchor')!r} != footer anchor {footer['anchor']!r}")

    ref_identity = ref_manifest.get("identity") or {}
    if ref_identity.get("model") != model:
        problems.append(f"{tag}: HINT_EVIDENCE_IDENTITY_MODEL_MISMATCH identity.model={ref_identity.get('model')!r} != tag model {model!r}")
    m_vllm = ref_identity.get("vllm")
    if not (m_vllm == vllm or (isinstance(m_vllm, str) and m_vllm.startswith(vllm + "-"))):
        problems.append(f"{tag}: HINT_EVIDENCE_IDENTITY_VLLM_MISMATCH identity.vllm={m_vllm!r} does not bind to tag vllm {vllm!r}")
    expected_class = _topology_class(footer["topology"])
    if ref_identity.get("topology") != expected_class:
        problems.append(f"{tag}: HINT_EVIDENCE_IDENTITY_TOPOLOGY_CLASS_MISMATCH identity.topology="
                         f"{ref_identity.get('topology')!r} != topology class {expected_class!r}")
    tp_label = _TP_LABEL_RE.search(footer["topology"])
    if tp_label is not None and ref_identity.get("tp") != int(tp_label.group(1)):
        problems.append(f"{tag}: HINT_EVIDENCE_IDENTITY_TP_MISMATCH identity.tp={ref_identity.get('tp')!r} "
                         f"!= TP{tp_label.group(1)} in topology label {footer['topology']!r}")

    identity_sha = _canonical_identity_sha256(ref_identity)
    if identity_sha != footer["identity_sha256"]:
        problems.append(f"{tag}: HINT_EVIDENCE_IDENTITY_SHA_MISMATCH {identity_sha} != {footer['identity_sha256']}")

    ref_cert_item = (ref_manifest.get("evidence") or {}).get("certificate")
    ref_cert_path = ref_cert_item.get("path") if isinstance(ref_cert_item, dict) else None
    if ref_cert_path != footer["certificate_ref"]:
        problems.append(f"{tag}: HINT_EVIDENCE_CERTIFICATE_REF_MISMATCH footer.certificate_ref="
                         f"{footer['certificate_ref']!r} != manifest evidence.certificate.path={ref_cert_path!r}")
    else:
        repo_root_fd = os.open(str(ROOT), os.O_RDONLY | os.O_DIRECTORY)
        try:
            r_cert = _cgate().resolve_and_stat_evidence(repo_root_fd, ROOT, manifest_ref_abs.parent,
                                                         footer["certificate_ref"], expect_dir=False)
        finally:
            os.close(repo_root_fd)
        if r_cert["status"] != "ok":
            problems.append(f"{tag}: HINT_EVIDENCE_CERTIFICATE_UNSAFE_OR_MISSING certificate_ref "
                             f"{footer['certificate_ref']!r} resolve 실패(status={r_cert['status']})")
        elif r_cert["sha256"] != footer["certificate_sha256"]:
            problems.append(f"{tag}: HINT_EVIDENCE_CERTIFICATE_SHA_MISMATCH {r_cert['sha256']} != {footer['certificate_sha256']}")

    return footer, problems


def cmd_verify(a: argparse.Namespace) -> int:
    _require_promotion_authorization("verify", a.manifest)
    terms = load_pii_terms()
    problems: list[str] = []
    tags = existing_hint_tags()
    idx = _load_index()
    idx_tags = {e["tag"] for e in idx["hints"]}

    for t in tags:
        typ = git("cat-file", "-t", t, check=False).stdout.strip()
        if typ != "tag":
            problems.append(f"{t}: annotated 아님(type={typ})")
            continue
        obj = git("cat-file", "tag", t).stdout
        header, _, tbody = obj.partition("\n\n")  # 헤더(tagger 이메일 정당) / 본문(이메일 금지) 분리
        h = (scan_text(header, terms, skip_generic=frozenset({"email"}))
             + scan_text(tbody, terms))
        if h:
            problems.append(f"{t}: 태그 오브젝트 PII {h}")
        if "TODO(judgment" in obj:
            problems.append(f"{t}: 본문에 미완 TODO")
        if t not in idx_tags:
            problems.append(f"{t}: index.json 미등재")
    for e in idx["hints"]:
        if e["tag"] not in tags:
            problems.append(f"index 에 {e['tag']} 있으나 태그 없음")

    # blocker 2 (cycle 2) -- EVERY hint tag's evidence is validated purely from its own footer,
    # never only the tag a particular --manifest happens to target.
    for t in tags:
        _footer, ev_problems = _validate_hint_tag_evidence(t, "hint_verify")
        problems.extend(ev_problems)

    for bp in sorted(ROOT.glob("output/*/build_patches/*.sh")):
        h = scan_text(bp.read_text(encoding="utf-8", errors="ignore"), terms)
        if h:
            problems.append(f"{bp.relative_to(ROOT)}: build_patch PII {h}")

    if a.check_origin:
        r = git("ls-remote", "--tags", "origin", "last-good-*", check=False)
        if r.returncode == 0 and r.stdout.strip():
            problems.append("origin 에 last-good-* 태그 존재(로컬 전용이어야 함):\n" + r.stdout.strip())

    if terms is None:
        problems.append("pii_terms.txt 부재 → generic 패턴만으로 스캔(축소 커버리지)")

    if problems:
        print("[hint_tag] VERIFY FAIL:")
        for p in problems:
            print("  -", p)
        return 1
    print(f"[hint_tag] VERIFY PASS: hint 태그 {len(tags)}개 · index 정합 · 태그오브젝트/build_patches PII-clean.")
    return 0


def _require_all_hint_tags_evidence_valid(action: str) -> None:
    """Full evidence-binding verification of EVERY existing hint tag (blocker 2, cycle 2) --
    dies atomically (stable JSON, BEFORE any caller-side write/print/network) if ANY tag's
    self-referenced evidence is missing/forged/drifted."""
    problems: list[str] = []
    for t in existing_hint_tags():
        _footer, ev_problems = _validate_hint_tag_evidence(t, action)
        problems.extend(ev_problems)
    if problems:
        _die_binding(action, ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "one or more hint tags carry missing/forged/drifted evidence -- refusing "
                      "(no partial rewrite/push):\n  " + "\n  ".join(problems)},
                     None, None)


# ── push (선별) ──────────────────────────────────────────────────────────────
def cmd_push(a: argparse.Namespace) -> int:
    _require_promotion_authorization("push", a.manifest)
    _require_all_hint_tags_evidence_valid("hint_push")  # full evidence-binding verification, before dry-run/apply
    refspec = "refs/tags/hint/*"
    print(f"[hint_tag] 선별 배포 refspec: git push {a.remote} \"{refspec}\"  (hint 태그만 · --tags 금지)")
    if not a.apply:
        print("[hint_tag] DRY-RUN (관례상 push 는 사용자가 직접). 실제 배포는 --apply.")
        git("push", "--dry-run", a.remote, refspec, check=False)
        return 0
    git("push", a.remote, refspec)
    print("[hint_tag] pushed refs/tags/hint/* (last-good-* 미포함).")
    return 0


# ── match (근-미스 발견) ──────────────────────────────────────────────────────
def cmd_match(a: argparse.Namespace) -> int:
    idx = _load_index()
    tgt = (a.vllm, canonicalize(a.model) or a.model, a.arch)
    scored = []
    for e in idx["hints"]:
        d = (e["vllm"] == tgt[0], e["model"] == tgt[1], e["arch"] == tgt[2])
        scored.append((sum(d), e, d))
    scored.sort(key=lambda x: -x[0])
    if not scored:
        print("[hint_tag] (인덱스 비어있음)")
        return 0
    for score, e, d in scored:
        guide = []
        if d == (True, True, True):
            guide.append("정확 일치 — 그래도 네 스모크로 재검증")
        else:
            if not d[1]:
                guide.append("다른 모델→서빙전략 독립(참고만)")
            if not d[0]:
                guide.append("다른 vLLM→릴리즈노트 재확인(carry-forward ✗)")
            if not d[2]:
                guide.append("다른 arch→빌드트랙·벽지도 이식가능·KV/gmu/TORCH_CUDA_ARCH 재도출")
        print(f"[{score}/3] {e['tag']}  ·  {' · '.join(guide)}")
    return 0


# ── reverify (currency 스탬프) ────────────────────────────────────────────────
def cmd_reverify(a: argparse.Namespace) -> int:
    _require_promotion_authorization("reverify", a.manifest)
    manifest, _ = _load_manifest_for_binding("hint_reverify", a.manifest)
    idx = _load_index()
    entry = next((e for e in idx["hints"] if e["tag"] == a.tag), None)
    if entry is None:
        die(f"[hint_tag] FAIL: {a.tag} 가 index 에 없음.")
    _, vllm, model, arch = a.tag.split("/")
    _require_hint_promotion_target("hint_reverify", manifest, tag=a.tag,
                                    topology=entry.get("topology", ""), anchor=entry.get("anchor", ""),
                                    vllm=vllm, model=model)
    # blocker 2 (cycle 2) -- also validate the TARGET tag's own self-referenced evidence (not just
    # the supplied --manifest's binding to it) before mutating the index's last_verified stamp.
    _footer, ev_problems = _validate_hint_tag_evidence(a.tag, "hint_reverify")
    if ev_problems:
        _die_binding("hint_reverify", ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "target tag's self-referenced evidence is missing/forged/drifted:\n  "
                      + "\n  ".join(ev_problems)},
                     manifest.get("identity"), manifest.get("task_class"))
    reachable = git("cat-file", "-e", entry["anchor"] + "^{commit}", check=False).returncode == 0
    entry["last_verified"] = date.today().isoformat()
    entry["anchor_reachable"] = reachable
    _save_index(idx)
    print(f"[hint_tag] reverify {a.tag}: anchor_reachable={reachable} · last_verified 스탬프.")
    return 0


# ── reindex (태그 = 진실원천 → index.json + HINTS.md 재생성) ──────────────────
def cmd_reindex(a: argparse.Namespace) -> int:
    """전 hint 태그에서 index.json + HINTS.md 카탈로그를 재생성한다(브랜치 간 드리프트 정합).
    currency 필드(status·superseded_by·last_verified·큐레이트 related)는 기존 index 에서 보존.
    binding 필드(topology·anchor)는 footer(=진실원천)에서만 재구성한다 -- 기존 index 신뢰 안 함
    (blocker 2 remediation)."""
    _require_promotion_authorization("reindex", a.manifest)
    # Every tag's evidence validated FIRST, atomically (footer self-consistency + safe manifest_ref/
    # certificate_ref resolution + digest match + referenced-manifest promotion-ready re-evaluation)
    # -- any problem dies here, before index.json/HINTS.md are touched at all (no partial rewrite,
    # cycle-2 remediation: a syntactically valid-but-forged footer must not slip through).
    tags = sorted(existing_hint_tags())
    footers: dict[str, dict[str, str]] = {}
    problems: list[str] = []
    for t in tags:
        footer, ev_problems = _validate_hint_tag_evidence(t, "hint_reindex")
        if footer is not None:
            footers[t] = footer
        problems.extend(ev_problems)
    if problems:
        _die_binding("hint_reindex", ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "one or more hint tags carry missing/forged/drifted evidence -- refusing "
                      "(no partial rewrite):\n  " + "\n  ".join(problems)},
                     None, None)
    idx = _load_index()
    prev = {e["tag"]: e for e in idx["hints"]}
    hints = []
    for t in tags:
        _, vllm, model, arch = t.split("/")
        footer = footers[t]
        brief, _old_topology, related = _parse_tag_body(t)
        p = prev.get(t, {})
        e = {
            "tag": t, "vllm": vllm, "model": model, "arch": arch,
            "topology": footer["topology"],
            "brief": brief or p.get("brief", ""),
            "anchor": footer["anchor"],
            "related": p.get("related") or related,  # 큐레이트(finalize) 우선, 없으면 본문 파싱
            "status": p.get("status", "active"),
            "last_verified": p.get("last_verified", date.today().isoformat()),
        }
        if p.get("superseded_by"):
            e["superseded_by"] = p["superseded_by"]
        hints.append(e)
    idx["hints"] = hints
    _save_index(idx)
    _hints_regen(hints)
    dropped = sorted(set(prev) - set(tags))
    print(f"[hint_tag] reindex: {len(hints)} 태그 → index.json + HINTS.md 재생성(currency 보존)."
          + (f"  제거(태그없음): {dropped}" if dropped else ""))
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="hint/<vllm>/<model>/<arch> 레시피-힌트 태그 관리")
    sub = ap.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("create", help="태그 검증 + 레시피 스캐폴드(TODO 슬롯)")
    c.add_argument("--tag", required=True)
    c.add_argument("--commit", default="HEAD")
    c.add_argument("--topology", required=True, help="예: 'multi 2노드 TP2' | 'single 1노드'")
    c.add_argument("--brief", default="")
    c.add_argument("--related", default="")
    c.add_argument("--from-resolved", default="resolved.json")
    c.add_argument("--allow-new-slug", action="store_true")
    c.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    c.set_defaults(fn=cmd_create)

    f = sub.add_parser("finalize", help="PII fail-closed 스캔 + annotated 태그 + 인덱스")
    f.add_argument("--tag", required=True)
    f.add_argument("--recipe", required=True)
    f.add_argument("--commit", default="HEAD")
    f.add_argument("--topology", required=True)
    f.add_argument("--related", default="")
    f.add_argument("--tagger-name", default="")
    f.add_argument("--tagger-email", default="")
    f.add_argument("--allow-new-slug", action="store_true")
    f.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    f.set_defaults(fn=cmd_finalize)

    v = sub.add_parser("verify", help="릴리즈 게이트(태그오브젝트/build_patches PII·인덱스 정합)")
    v.add_argument("--check-origin", action="store_true", help="origin 에 last-good-* 없음 확인(네트워크)")
    v.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    v.set_defaults(fn=cmd_verify)

    p = sub.add_parser("push", help="선별 배포 refs/tags/hint/* (--tags 금지)")
    p.add_argument("--remote", default="origin")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    p.set_defaults(fn=cmd_push)

    m = sub.add_parser("match", help="근-미스 발견(축별 이식 가이드) -- read-only, ungated")
    m.add_argument("--vllm", required=True)
    m.add_argument("--model", required=True)
    m.add_argument("--arch", required=True)
    m.set_defaults(fn=cmd_match)

    r = sub.add_parser("reverify", help="핀 자산 reachability + last_verified 스탬프")
    r.add_argument("--tag", required=True)
    r.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    r.set_defaults(fn=cmd_reverify)

    ri = sub.add_parser("reindex", help="전 hint 태그에서 index.json+HINTS.md 재생성(브랜치 드리프트 정합·currency 보존)")
    ri.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    ri.set_defaults(fn=cmd_reindex)

    a = ap.parse_args()
    if a.cmd != "match":
        require_git_repository()
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
