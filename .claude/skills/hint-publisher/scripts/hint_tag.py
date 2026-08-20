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
import fnmatch
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
from datetime import date, datetime
from pathlib import Path

TAG_SHAPE = re.compile(r"^hint/[^/]+/[^/]+/[^/]+$")
HINTS_MARKER = "<!-- hint-index:rows -->"

# Canonical model-slug per family (hardening #5 — prevents slug sprawl that would break
# `git tag -l 'hint/*/<slug>/*'`). Value = accepted spellings (canonical MUST be first-listed
# via the dict key). Register a new family here (HITL) rather than minting drive-by slugs.
CANONICAL_SLUGS: dict[str, set[str]] = {
    "deepseek-v4-flash": {"deepseek-v4-flash", "ds4flash", "deepseek-v4-flash-dspark"},
    # 0731 = 정식판. 프리뷰와 **가중치가 다르다**(index 해시부터 상이) → 별칭이 아니라 별도 family.
    #   이미 `hint/0.26.0/deepseek-v4-flash-0731/gb10x2` 가 push 된 상태라 정본 철자는 이것으로 고정된다.
    #   사다리(1칸 노멀 · 2칸 1M · 3칸 dspark)는 **arch 슬롯**으로 갈라지므로 슬러그는 하나로 족하다.
    "deepseek-v4-flash-0731": {"deepseek-v4-flash-0731", "ds4f0731"},
    "gpt-oss-120b": {"gpt-oss-120b", "gptoss120b", "gpt-oss"},
    "qwen3-next-80b-bf16": {"qwen3-next-80b-bf16", "qwen3next80b", "qwen3-next-80b"},
    "qwen3.5-122b-a10b-nvfp4": {"qwen3.5-122b-a10b-nvfp4", "qwen35-122b-nvfp4"},
    "skt-a.x-4.0-72b": {"skt-a.x-4.0-72b", "skt-ax-72b"},
    "gemma-3-27b": {"gemma-3-27b", "gemma3-27b"},
    # Tencent Hy3-295B(21B active + 3.8B MTP). `hint/0.24.0/hy3/gb10` 이 이미 push 된 상태라
    #   정본 철자는 `hy3` 로 고정된다(위 0731 주석과 같은 근거). 변종 사다리(기준선·spec 축·
    #   CUDA graph·block-size·attention backend)는 **arch 슬롯**으로 갈라지므로 슬러그는 하나로 족하다.
    "hy3": {"hy3", "hy3-295b", "hunyuan3"},
}

# Generic PII patterns (belt-and-suspenders atop pii_terms.txt literals). Narrow on
# purpose so versions ("2.11.0" = 3 octets) don't false-positive as IPv4.
GENERIC_PII: list[tuple[str, re.Pattern]] = [
    ("private-ipv4", re.compile(r"\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}(?:\.\d{1,3})?")),
    ("email", re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")),
    ("abs-op-path", re.compile(r"/(?:mnt|home)/[A-Za-z0-9._/-]+")),
    ("spark-host", re.compile(r"spark-[0-9a-f]{3,}")),
]

# Document section numbers ("§10.1.2", "#### 10.1 ...") are NOT private IPv4 addresses. The
# `10\.\d{1,3}\.` branch above cannot tell them apart on shape alone, so the discriminator is the
# text IMMEDIATELY BEFORE the match, within the same line: a `§` sigil, or a markdown heading
# marker. Measured on plan_26081410: 14/14 matches were section numbers, 0 genuine (plan_26081514
# §6.1). Narrowing a safety pattern is only admissible with a no-loss proof -- the merge gate was
# "genuine IP detections must stay at 169" (plan_26081516 §4 H1), and the negative fixture in
# claim_predicates C2 is the tripwire that forces review if anyone widens or narrows this again.
_SECTION_ANCHOR = re.compile(r"(?:§\s*|^#{1,6}\s+)$")
# Only the IPv4 branch is shape-ambiguous with section numbers; email/abs-path/spark-host are not.
_ANCHOR_EXCLUDED: frozenset[str] = frozenset({"private-ipv4"})


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
TEMPLATE_FILE = ROOT / ".claude" / "skills" / "hint-publisher" / "templates" / "hint_recipe.template.md"
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
    "pin-legacy": "hint_reindex",   # 카탈로그 정합 계열 — reindex 와 같은 권한면
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
        try:
            import completion_gate as _cg           # 서브 배포(동거 사본) 경로
        except ModuleNotFoundError:
            # 메인 레이아웃: completion_gate.py 는 .claude/policies/runtime/ 에 있고 scripts/ 옆에
            # 없다. 이 파일은 그 정본 경로를 이미 COMPLETION_GATE_SCRIPT 로 알고 subprocess 호출에
            # 쓰면서(_authorize) 여기서만 bare import 를 해, **메인에서 finalize/verify 가
            # ModuleNotFoundError 로 죽었다**(2026-07-31 발견 — Phase-3 로 추가된 identity-sha256
            # 경로가 메인에서 한 번도 실행되지 않았다). 정본 경로에서 직접 적재해 두 레이아웃을 모두 지원한다.
            import importlib.util as _ilu
            if not COMPLETION_GATE_SCRIPT.is_file():
                die("[hint_tag] FAIL: completion_gate.py 를 찾을 수 없다 "
                    f"(동거 사본 ✗ · {COMPLETION_GATE_SCRIPT} ✗)")
            _spec = _ilu.spec_from_file_location("completion_gate", COMPLETION_GATE_SCRIPT)
            _cg = _ilu.module_from_spec(_spec)
            _spec.loader.exec_module(_cg)
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

        cert_path_str = _binding_artifact_path(manifest)
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


def _is_section_anchored(text: str, start: int) -> bool:
    """True when the match at `start` is preceded, ON ITS OWN LINE, by a `§` sigil or a markdown
    heading marker -- i.e. it is a document section number, not an address. The prefix is cut at
    the line start on purpose: searching the whole preceding text would let a `§` sitting alone on
    some earlier line suppress a genuine hit far below it."""
    line_start = text.rfind("\n", 0, start) + 1
    return _SECTION_ANCHOR.search(text[line_start:start]) is not None


def scan_text(text: str, terms: list[str] | None, skip_generic: frozenset[str] = frozenset()) -> list[str]:
    """Scan for pii_terms literals + generic patterns. `skip_generic` drops named generic
    patterns — the tagger identity check skips 'email' (a tagger MUST have an email; we only
    forbid it carrying a KNOWN-PII literal like a personal handle/domain, not being an email).

    Anchored section numbers are excluded for `_ANCHOR_EXCLUDED` patterns. Note this walks EVERY
    match rather than taking `search`'s first one: with an exclusion in play, stopping at match #1
    would let a leading false positive mask a genuine address later in the same text."""
    hits = []
    for t in terms or []:
        if t and t in text:
            hits.append(f"term:{t}")
    for name, pat in GENERIC_PII:
        if name in skip_generic:
            continue
        anchored_excluded = name in _ANCHOR_EXCLUDED
        for m in pat.finditer(text):
            if anchored_excluded and _is_section_anchored(text, m.start()):
                continue
            hits.append(f"{name}:{m.group(0)}")
            break
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
    slug_src = require_derived_slug(model, vllm, arch, a.hf_repo, a.model_path)
    print(f"[hint_tag] 슬러그 '{model}' 확인 (출처: {slug_src})")
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    manifest, _ = _load_manifest_for_binding("hint_create", a.manifest)
    _require_hint_promotion_target("hint_create", manifest, tag=a.tag, topology=a.topology,
                                    anchor=anchor, vllm=vllm, model=model)
    _require_serving_evidence("hint_create", manifest)

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
    follow_up = ["python3", ".claude/skills/hint-publisher/scripts/hint_tag.py", "finalize",
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
    require_derived_slug(model, vllm, arch, a.hf_repo, a.model_path)
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

    lint_problems = lint_body(body)          # D10 L1-L5 — 일관성의 실제 보장(fail-closed)
    if lint_problems:
        die("[hint_tag] FAIL: 레시피 정보구조 린트 위반 — 태그가 나가지 않는다.\n        "
            + "\n        ".join(lint_problems)
            + "\n        (자유 기술은 `## 8. comment` 절에 — 그 칸만 검사 제외)")

    # perf_waiver(성능 REFUTE 사람승인)가 있으면 경고가 본문에 실제로 담겼는지 fail-closed 확인.
    _require_perf_warning("hint_finalize", manifest, body)

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

    if getattr(a, "no_index", False):
        print("[hint_tag] seal 완료 — 색인은 중앙이 `index --tag` 로 수행한다(D8 권한 비대칭).")
        return 0
    _require_central("finalize(색인 갱신 포함)")
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
        # 부재(not_found)와 위험(symlink/escape/…)을 **가른다** — 부재는 "이 체크아웃에 증거가 없다"
        # 이고, 수신자 클론에서는 그게 정상이다(증거는 배포되지 않는다). 같은 등급으로 묶으면
        # 수신자 쪽에서 전량이 차단으로 읽혀 신호가 죽는다(plan_26082017 §10.4 · 사용자 결정 α).
        _code = ("HINT_EVIDENCE_MANIFEST_REF_ABSENT" if r_manifest["status"] == "not_found"
                 else "HINT_EVIDENCE_MANIFEST_REF_UNSAFE_OR_MISSING")
        problems.append(f"{tag}: {_code} manifest_ref "
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

    ref_cert_path = _binding_artifact_path(ref_manifest)
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
            _code = ("HINT_EVIDENCE_CERTIFICATE_REF_ABSENT" if r_cert["status"] == "not_found"
                     else "HINT_EVIDENCE_CERTIFICATE_UNSAFE_OR_MISSING")
            problems.append(f"{tag}: {_code} certificate_ref "
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
    # evidence-binding 분류는 classify_evidence_problems 단일 권위(계약 v2 §5).
    # verify 는 릴리즈 게이트이지만 v1 빈티지를 차단하지 않는다 — 그러면 신규 태그 발행이
    # 레거시 부채에 영구히 인질로 잡힌다(계약 §4). 변조는 여기서도 그대로 차단된다.
    _ev_blocking, _ev_legacy, _ev_drift, _ev_unver = classify_evidence_problems(
        [(t, _validate_hint_tag_evidence(t, "hint_verify")[1]) for t in tags])
    problems.extend(_ev_blocking)
    # 발행자 평면(verify=릴리즈 게이트)에서는 **부재도 차단**이다 — 발행자는 자기가 주장하는 증거를
    # 갖고 있어야 한다. 수신자 평면(collect·reindex)에서만 부재를 경고로 낮춘다(사용자 결정 α).
    problems.extend(f"{x}: HINT_EVIDENCE_REF_ABSENT 참조 증거가 이 체크아웃에 없다 — "
                    "발행자는 증거를 보유해야 한다" for x in _ev_unver)
    problems.extend(f"{x}: HINT_TAG_UNSEALED {_r}" for x in tags
                    if (_r := unsealed_reason(x)))
    if _ev_legacy:
        print(f"[hint_tag] ⚠ v1 빈티지 {len(_ev_legacy)}개(footer 이전 · SHA 핀 일치) — 경고.",
              file=sys.stderr)

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


def _require_serving_evidence(action: str, manifest: dict) -> None:
    """계약 v2 §3 — **발행 가능 시점**의 실질 검사. 이것이 §2 위협의 주 방어선이다.

    위협: "서빙이 실패했는데도 에이전트가 사용자를 속여 '서빙되었다'고 허위 기재한 정보가 배포되는 것".
    v1 게이트는 형식(footer)만 보고 이걸 **아예 검사하지 않았다**. v2 는 두 조건을 강제한다.

      A. 서빙전략 달성 — runtime.health_ok AND functional_smoke_passed (컨테이너 oom_killed ✗)
      B. 정량지표 확보 — 인증서에 lite_included: true 와 lite 실측 열

    B 가 lite 기준인 이유: lite 는 서빙 성공 시 자동 수행되는 **암시적 필수 계측**이고, full 은 선택이다.
    full ⊇ lite 불변식(계약 §3.1) 때문에 full 을 돌렸다면 B 는 자동 충족된다.
    A 없이 B 는 성립할 수 없으므로(서빙이 안 되면 측정 대상이 없다), B 는 사실상
    **'서빙되었다'의 정량 증거**다.
    """
    problems: list[str] = []
    rt = manifest.get("runtime") if isinstance(manifest, dict) else None
    if not isinstance(rt, dict):
        problems.append("runtime 블록 부재 — 서빙 성공 증거 없음")
    else:
        if rt.get("health_ok") is not True:
            problems.append("runtime.health_ok != true")
        if rt.get("functional_smoke_passed") is not True:
            problems.append("runtime.functional_smoke_passed != true")
        if any(c.get("oom_killed") for c in (rt.get("containers") or []) if isinstance(c, dict)):
            problems.append("컨테이너가 oom_killed — 서빙 성공으로 볼 수 없다")

    # B: lite 정량지표. 통상은 인증서에서 읽는다.
    #    ★ 단 **perf_waiver 경로(REFUTE)** 에서는 인증서가 애초에 존재할 수 없다 —
    #      publish_benchmark_record 가 PASS 때만 인증서를 내기 때문이다(그 규칙은 유지한다).
    #      그러나 lite 실측 자체는 **항상 발행되는 bench_report** 에 실려 있으므로,
    #      waiver 경로에서는 리포트를 B 의 근거로 삼는다. "증거가 없다"가 아니라
    #      "증거가 다른 문서에 있다" 이므로 요구 강도를 낮추는 것이 아니다.
    ev = manifest.get("evidence") if isinstance(manifest.get("evidence"), dict) else {}
    waiver = ((manifest.get("benchmark") or {}).get("perf_waiver")
              if isinstance(manifest.get("benchmark"), dict) else None)
    cert_rel = (ev.get("certificate") or {}).get("path")
    if not cert_rel and waiver:
        rep_rel = (ev.get("bench_report") or {}).get("path")
        if not rep_rel:
            problems.append("perf_waiver 경로인데 evidence.bench_report 도 없다 — lite 근거 부재")
        else:
            rep_path = (ROOT / "docs" / "_evidence" / rep_rel).resolve()
            try:
                rtext = rep_path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                rtext = ""
                problems.append(f"bench_report 를 읽을 수 없다: {rep_rel}")
            if rtext:
                if "lite 지표" not in rtext:
                    problems.append("bench_report 에 lite 지표 절 부재 — full ⊇ lite 가 깨졌다")
                if not re.search(r"gen tokens/sec[^|]*\|\s*[0-9]", rtext):
                    problems.append("bench_report 의 lite warm gen 실측값 부재")
    elif not cert_rel:
        problems.append("evidence.certificate 부재 — lite 정량지표를 확인할 수 없다")
    else:
        cert_path = (ROOT / "docs" / "_evidence" / cert_rel).resolve()
        try:
            text = cert_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            text = ""
            problems.append(f"인증서를 읽을 수 없다: {cert_rel}")
        if text:
            if not re.search(r"(?m)^lite_included:\s*true\b", text):
                problems.append("인증서 lite_included != true — lite 정량지표 미확보"
                                "(full ⊇ lite 불변식이 깨졌거나 lite 수집 실패)")
            if not re.search(r"(?m)^lite_gen_tps_warm:\s*[0-9]", text):
                problems.append("인증서 lite_gen_tps_warm 실측값 부재")
    if problems:
        _die_binding(action, ["HINT_SERVING_EVIDENCE_INSUFFICIENT"],
                     {"HINT_SERVING_EVIDENCE_INSUFFICIENT":
                      "hints/HINT_ISSUANCE_CONTRACT.md §3 발행 조건 미충족 — "
                      "서빙 성공과 lite 정량지표가 모두 증명돼야 한다:\n  " + "\n  ".join(problems)},
                     manifest.get("identity") if isinstance(manifest, dict) else None,
                     manifest.get("task_class") if isinstance(manifest, dict) else None)


def _binding_artifact_path(manifest: dict) -> "str | None":
    """footer 가 해시로 묶을 **계측 산출물 경로**의 단일 소유자.

    통상은 인증서다. 단 perf_waiver(성능 REFUTE 사람승인) 경로에는 인증서가 애초에 존재할 수
    없으므로(publish_benchmark_record 가 PASS 때만 발행 — 그 규칙은 유지) **항상 발행되는
    bench_report** 를 바인딩 대상으로 삼는다. 요구를 낮추는 게 아니라 대상 문서가 다를 뿐이다.

    ★ 이 판정이 finalize·verify 두 곳에 **각각 박혀 있어** 한쪽만 고치면 즉시 불일치가 난다
      (2026-08-01 실측: finalize 는 리포트로 묶었는데 verify 는 인증서와 비교해 FAIL).
      그래서 한 함수로 모은다 — 오늘 반복해서 확인한 "같은 가정이 여러 곳" 결함 계열이다.
    """
    ev = manifest.get("evidence") if isinstance(manifest, dict) else None
    ev = ev if isinstance(ev, dict) else {}
    cert = ev.get("certificate")
    path = cert.get("path") if isinstance(cert, dict) else None
    if path:
        return path
    waiver = ((manifest.get("benchmark") or {}).get("perf_waiver")
              if isinstance(manifest, dict) and isinstance(manifest.get("benchmark"), dict) else None)
    if waiver:
        rep = ev.get("bench_report")
        return rep.get("path") if isinstance(rep, dict) else None
    return None


PERF_WARNING_MARKER = "PERF-WARNING"


def _require_perf_warning(action: str, manifest: dict, recipe_text: str) -> None:
    """perf_waiver 가 있으면 **배포 산출물에 경고가 실제로 담겼는지** 확인한다(fail-closed).

    waiver 의 대가가 경고 플래그인데 그 경고가 본문에 없으면 waiver 는 그냥 게이트 우회가 된다.
    그래서 여기서 두 가지를 강제한다:
      1) `PERF-WARNING` 마커 존재 — 기계가 찾을 수 있는 고정 토큰
      2) manifest 의 warning_flag 텍스트가 본문에 실제로 포함 — 사람이 선언한 문구 그대로
    (2026-08-01 사용자 결정: "루프-언틸-던을 사람 지시로 깨되 hint 에 warning flag 를 기록한다")
    """
    waiver = ((manifest.get("benchmark") or {}).get("perf_waiver")
              if isinstance(manifest, dict) and isinstance(manifest.get("benchmark"), dict) else None)
    if not waiver:
        return
    problems = []
    if PERF_WARNING_MARKER not in recipe_text:
        problems.append(f"본문에 {PERF_WARNING_MARKER} 마커가 없다")
    wf = (waiver.get("warning_flag") or "").strip()
    if wf and wf not in recipe_text:
        problems.append("manifest.perf_waiver.warning_flag 문구가 본문에 그대로 실려 있지 않다")
    if problems:
        _die_binding(action, ["HINT_PERF_WARNING_MISSING"],
                     {"HINT_PERF_WARNING_MISSING":
                      "perf_waiver(성능 REFUTE 승인)가 있는데 배포 본문에 경고가 없다 — "
                      "waiver 의 대가가 경고이므로 경고 없는 waiver 는 단순 우회다:\n  "
                      + "\n  ".join(problems)},
                     manifest.get("identity") if isinstance(manifest, dict) else None,
                     manifest.get("task_class") if isinstance(manifest, dict) else None)


LEGACY_PINS_FILE = ROOT / "hints" / "legacy_v1_pins.json"
DRIFT_PINS_FILE = ROOT / "hints" / "evidence_drift_pins.json"


def _load_drift_pins() -> dict:
    """발행 **후** 매니페스트가 승인된 편집으로 바뀐 태그의 등재부.

    왜 필요한가(2026-08-18 실제 발생): 사용자 지시로 `approved_by: coag-ash → AhnSangHun` 일괄
    개명을 하면서 plan digest 재계산까지 돌았고, 그 순간 16개 태그의 footer `manifest_sha256` 이
    전부 어긋났다. 해시는 **승인된 개명과 변조를 구분하지 못한다** — 그게 해시의 본분이다.
    그런데 계약 §2 가 지목한 유일한 위협은 *"서빙 실패를 성공으로 허위기재해 배포하는 것"* 이고,
    이 드리프트는 그 표면에 닿지 않았다(identity·certificate·verdict·health·smoke 전부 보존).

    v1 이 형식으로 차단하고 증거를 검사하지 않은 실수를 v2 가 고쳤는데, `drifted` 판정만은
    여전히 **바이트 형식**으로 내려지고 있었다. 이 핀이 그 마지막 칸을 증거 기준으로 옮긴다.

    **핀은 면제가 아니라 동결이다** — 등재된 바이트에서 *더* 바뀌면 다시 차단된다."""
    if not DRIFT_PINS_FILE.is_file():
        return {}
    try:
        with open(DRIFT_PINS_FILE, encoding="utf-8") as f:
            return json.load(f).get("pins", {})
    except (OSError, json.JSONDecodeError):
        return {}


_DRIFT_SHA_RE = re.compile(r"HINT_EVIDENCE_MANIFEST_SHA_MISMATCH\s+([0-9a-f]{64})\s*!=\s*([0-9a-f]{64})")


def _is_evidence_preserving_drift(tag: str, ev_problems: list, pins: dict) -> bool:
    """증거-보전 드리프트인가. 네 조건을 **전부** 만족해야 한다.

    ① 문제가 `MANIFEST_SHA_MISMATCH` **하나뿐**이다.
       — identity/certificate/verdict 불일치는 각자 **다른 코드**로 나오므로, 이 조건 하나가
         "계측·정체성·판정은 그대로였다"를 구조적으로 보장한다.
    ② 그 태그가 등재돼 있다.                    (사람 승인)
    ③ 등재된 footer 기대값이 실제 footer 와 같다. (핀이 다른 태그 것을 재활용하지 못한다)
    ④ 등재된 관측값이 **지금** 매니페스트와 같다. (등재 이후 추가 드리프트는 다시 차단)
    """
    if not ev_problems or len(ev_problems) != 1:
        return False
    m = _DRIFT_SHA_RE.search(ev_problems[0])
    if not m:
        return False
    observed, expected = m.group(1), m.group(2)
    pin = pins.get(tag)
    if not isinstance(pin, dict):
        return False
    return (pin.get("footer_manifest_sha256") == expected
            and pin.get("observed_manifest_sha256") == observed)



# 린터(L1–L5) 도입 시각. 이 시각 **이후** 발행분만 린트 준수를 강제한다 — 그 전에는 요구되지
# 않았고 "부재가 곧 허위는 아니다"(계약 §4 소급 금지와 같은 논리). tripwire 상수이므로 바꾸려면
# 리뷰가 필요하다. 근거: 린터 도입 커밋 daf63f3 = 2026-08-20T00:49:20Z (plan_26082009 D10).
LINT_ERA_EFFECTIVE_EPOCH = 1787186960


def _tagger_epoch(tag: str) -> int | None:
    tok = git("for-each-ref", "--format=%(taggerdate:raw)", f"refs/tags/{tag}",
              check=False).stdout.split()
    try:
        return int(tok[0])
    except (IndexError, ValueError):
        return None


def unsealed_reason(tag: str) -> str | None:
    """이 태그가 `seal` 산출물이 **아님**을 태그 오브젝트만으로 판정한다.

    ★ 외부 파일이 필요 없다 — 그래서 **증거가 배포되지 않는 수신자 평면에서도 그대로 작동하는
    유일한 검출기**다. digest 계열 검사는 증거 파일이 있어야 하므로 수신자에게는 항상 '부재'로
    떨어진다(plan_26082017 §10.4). 손으로 만든 footer 는 digest 로는 못 잡아도 여기서 잡힌다:
    `cmd_finalize` 가 lint_body 를 die 로 집행하므로, **린트를 못 넘는 본문은 seal 이 낸 것이
    아니다.** 2026-08-20 실증(태그 4건)."""
    ep = _tagger_epoch(tag)
    if ep is None or ep < LINT_ERA_EFFECTIVE_EPOCH:
        return None                                   # 린터 이전 발행분 — 소급하지 않는다
    if git("cat-file", "-t", tag, check=False).stdout.strip() != "tag":
        return "annotated 태그가 아니다 — hint 태그는 본문이 곧 페이로드다"
    body = git("cat-file", "tag", tag, check=False).stdout.partition("\n\n")[2]
    probs = lint_body(body)
    if probs:
        return (f"린트 위반 {len(probs)}건(예: {probs[0]}) — `seal` 은 이 본문으로 "
                "태그를 만들지 않는다(=도구를 거치지 않았다)")
    return None


def classify_evidence_problems(per_tag: list) -> tuple:
    """계약 v2 §5 분류의 **단일 권위**. 입력 [(tag, ev_problems)] → (blocking[], legacy_warn[]).

    push·reindex·reverify 세 곳이 각자 같은 판정을 복제하고 있었다(2026-07-31 발견). 그러면
    한 곳만 고쳤을 때 나머지가 남는다 — 이 프로젝트에서 같은 계열 사고가 이미 여러 번 났다
    (파서 두 벌 D4↔D6 · TP 오카운트 7사이트). **판정은 여기 하나뿐이다.**

      missing + 핀 SHA 일치  → legacy_warn (v1 빈티지 · 차단 ✗ · 계약 §4)
      missing + 핀 SHA 불일치 → blocking   (레거시를 손댔으면 v2 를 만족시켜라)
      missing + 핀 없음      → blocking   (v2 이후 신규는 footer 필수)
      forged / drifted       → blocking   (변조는 빈티지와 무관)
      *_REF_ABSENT 만        → unverifiable (참조 파일이 이 체크아웃에 없다 — 수신자 평면에선 정상)

    ★ `unverifiable` 은 "증거가 틀렸다"가 아니라 **"여기서는 대조할 수 없다"** 다. 증거(work-
    manifest)는 배포되지 않으므로 수신자 클론에서는 사실상 전량이 여기 떨어진다. 이를 blocking 과
    한 등급으로 묶으면 수신자 쪽 카탈로그가 통째로 경고가 되어 신호가 죽는다(plan_26082017 §10.4).
    대신 도구 미경유는 `unsealed_reason()` 이 **파일 없이** 잡으므로 검출력은 유지된다.
    """
    pins = _load_legacy_v1_pins()
    drift_pins = _load_drift_pins()
    blocking: list = []
    legacy_warn: list = []
    drift_warn: list = []
    unverifiable: list = []
    for tag, ev_problems in per_tag:
        if not ev_problems:
            continue
        only_missing = all("HINT_EVIDENCE_BINDING_MISSING" in p for p in ev_problems)
        pinned = pins.get(tag)
        if only_missing and pinned:
            try:
                cur = subprocess.run(["git", "rev-parse", "--verify", tag],
                                     capture_output=True, text=True).stdout.strip()
            except Exception:
                cur = ""
            if cur and cur == pinned:
                legacy_warn.append(tag)
                continue
            blocking.append(f"{tag}: v1 핀 SHA 불일치(pin={pinned[:12]} cur={cur[:12] or 'N/A'}) "
                            "— 레거시 태그가 변경됐다면 v2 evidence-binding 을 만족시켜야 한다")
            continue
        if _is_evidence_preserving_drift(tag, ev_problems, drift_pins):
            drift_warn.append(tag)          # 증거-보전 드리프트 · 등재분 (계약 §5 개정 2026-08-20)
            continue
        if all("_REF_ABSENT" in p for p in ev_problems):
            unverifiable.append(tag)        # 참조 파일 부재 only — 등급 분리(사용자 결정 α)
            continue
        blocking.extend(ev_problems)
    return blocking, legacy_warn, drift_warn, unverifiable


def _load_legacy_v1_pins() -> dict:
    """v2 발효 시점의 v1 빈티지 태그 SHA 핀(계약 §4). 부재 시 {} — 그러면 전 태그가 v2 강제다."""
    try:
        with open(LEGACY_PINS_FILE, encoding="utf-8") as f:
            return (json.load(f) or {}).get("pins") or {}
    except (OSError, ValueError):
        return {}


def _pins_effective_utc() -> str:
    """핀 목록의 v2 발효 시각. 부재/불량이면 빈 문자열 → pre-effective 판정을 하지 않는다."""
    try:
        with open(LEGACY_PINS_FILE, encoding="utf-8") as f:
            return str((json.load(f) or {}).get("effective_utc") or "")
    except (OSError, ValueError):
        return ""


def _tag_is_pre_effective(tag: str):
    """태그의 tagger 시각이 v2 발효보다 앞서면 True. 판정 불가면 None.

    ★ 이 함수는 **진단에만** 쓴다. 자동 핀에 쓰지 않는다 — tagger 날짜는 태그 오브젝트 안에 있어
      신규 위조 태그가 날짜를 소급해 빈티지를 참칭할 수 있다. 핀의 보안 가치는 '알려진 시점에
      사람이 열거했다' 는 데 있으므로, 등재는 언제나 사람 게이트를 통과해야 한다.
    """
    eff = _pins_effective_utc()
    if not eff:
        return None
    try:
        raw = subprocess.run(["git", "for-each-ref", "--format=%(taggerdate:iso-strict)",
                              f"refs/tags/{tag}"], capture_output=True, text=True).stdout.strip()
        if not raw:
            return None
        tagged = datetime.fromisoformat(raw)
        effective = datetime.fromisoformat(eff.replace("Z", "+00:00"))
    except (OSError, ValueError):
        return None
    return tagged < effective


def _legacy_remedy_hint(problems: list) -> str:
    """차단된 태그 중 pre-effective 인 것에 대해 **정확한 해소 명령**을 문자열로 만든다.

    종전엔 'binding 이 없다' 는 일반 문구뿐이라, 피어 호스트가 v2 발효 이전에 발행한 태그를
    뒤늦게 페치하면 사람이 legacy_v1_pins.json 을 **손으로 열어 편집**하는 수밖에 없었다
    (2026-08-01 hint/0.25.1/gemma-4-e2b-it/rtx5090 실제 발생). 핀 목록의 입력이 '로컬에 있던
    태그' 집합이라 구조적으로 재발한다 — 사람 게이트는 유지하되 경로는 제시한다.
    """
    lines = []
    for p in problems:
        if "HINT_EVIDENCE_BINDING_MISSING" not in p:
            continue
        tag = p.split(":", 1)[0].strip()
        if not tag.startswith("hint/") or _tag_is_pre_effective(tag) is not True:
            continue
        lines.append(
            f"  ↳ {tag} 는 tagger 시각이 v2 발효({_pins_effective_utc()})보다 **앞선다** = v1 빈티지 후보다.\n"
            f"    핀 목록이 로컬 태그만으로 만들어져 누락된 것일 수 있다(피어 호스트 발행분). 사람 확인 후:\n"
            f"      python3 .claude/skills/hint-publisher/scripts/hint_tag.py pin-legacy \\\n"
            f"        --tag {tag} --manifest <promotion-ready work-manifest>")
    return ("\n\n[hint_tag] v1 빈티지 후보 감지 — 해소 경로:\n" + "\n".join(lines)) if lines else ""


def cmd_pin_legacy(a: argparse.Namespace) -> int:
    """v2 발효 이전에 발행된 footer-없는 태그를 legacy_v1_pins 에 SHA 로 등재한다.

    세 조건을 **전부** 만족해야 등재한다(하나라도 어긋나면 거부, 부분 기록 없음):
      ① 태그에 evidence-binding footer 가 실제로 없다 (있으면 v2 태그이므로 핀 대상이 아니다)
      ② tagger 시각 < effective_utc (post-effective 태그의 빈티지 참칭 차단)
      ③ 아직 핀되어 있지 않다 (기존 핀 덮어쓰기 = 변조 탐지 무력화)
    """
    _require_promotion_authorization("pin-legacy", a.manifest)
    tag = a.tag
    if tag not in set(existing_hint_tags()):
        print(f"[hint_tag] FAIL: 존재하지 않는 태그: {tag}", file=sys.stderr)
        return 2

    footer, _ = _validate_hint_tag_evidence(tag, "hint_pin_legacy")
    if footer is not None:
        print(f"[hint_tag] FAIL: {tag} 에는 evidence-binding footer 가 있다 — v2 태그는 핀 대상이 아니다.",
              file=sys.stderr)
        return 2

    pre = _tag_is_pre_effective(tag)
    eff = _pins_effective_utc()
    if pre is not True:
        why = "판정 불가(tagger 시각/발효시각 해석 실패)" if pre is None else f"tagger 시각이 발효({eff}) 이후"
        print(f"[hint_tag] FAIL: {tag} 는 v1 빈티지가 아니다 — {why}. "
              "v2 태그는 evidence-binding 을 갖춰야 한다(핀으로 우회 불가).", file=sys.stderr)
        return 2

    try:
        with open(LEGACY_PINS_FILE, encoding="utf-8") as f:
            doc = json.load(f)
    except (OSError, ValueError) as e:
        print(f"[hint_tag] FAIL: 핀 목록을 읽을 수 없다: {e}", file=sys.stderr)
        return 2
    pins = doc.get("pins") or {}
    if tag in pins:
        print(f"[hint_tag] FAIL: {tag} 는 이미 핀되어 있다(pin={pins[tag][:12]}). "
              "덮어쓰기는 변조 탐지를 무력화하므로 거부한다.", file=sys.stderr)
        return 2

    sha = subprocess.run(["git", "rev-parse", "--verify", tag],
                         capture_output=True, text=True).stdout.strip()
    if not sha:
        print(f"[hint_tag] FAIL: {tag} 의 오브젝트 SHA 해소 실패", file=sys.stderr)
        return 2

    pins[tag] = sha
    doc["pins"] = dict(sorted(pins.items()))
    with open(LEGACY_PINS_FILE, "w", encoding="utf-8") as f:
        f.write(json.dumps(doc, ensure_ascii=False, indent=2) + "\n")
    print(f"[hint_tag] pin-legacy: {tag}\n  obj={sha}\n  근거=tagger 시각 < effective_utc({eff}) · footer 부재\n"
          f"  핀 총계={len(doc['pins'])}. 이후 SHA 가 바뀌면 v2 조건이 강제된다(변조 탐지 보존).")
    return 0


def _require_all_hint_tags_evidence_valid(action: str, tags: list | None = None) -> None:
    """모든 hint 태그의 evidence-binding 검증 — **계약 v2 분류**(hints/HINT_ISSUANCE_CONTRACT.md §5).

    v1 은 missing/forged/drifted 를 한 덩어리로 차단했다. 그 결과 evidence-binding footer 규약이
    Phase-3 에서 **나중에** 추가되면서, 그 이전 태그 23개가 이후의 모든 push 를 영구히 막았다.
    hint 태그의 목적은 토큰 이코노미이고(계약 §1), **정보가 적은 구버전은 결함이 아니다**.
    진짜 위협은 "서빙 실패를 성공으로 위장한 허위 배포"이지 형식 미비가 아니다(계약 §2).

    v2 분류:
      - 핀 목록에 있고 SHA 그대로  → v1 빈티지 → **경고**(차단 ✗)
      - 핀에 없음(= v2 이후 신규)   → **차단**(footer 필수)
      - 핀에 있는데 SHA 가 바뀜     → **차단**(레거시를 손댔으므로 v2 를 만족시켜야 한다)
      - forged / drifted           → **차단 유지**(변조는 빈티지와 무관)
    """
    targets = tags if tags is not None else existing_hint_tags()
    blocking, legacy_warn, drift_warn, unverifiable = classify_evidence_problems(
        [(t, _validate_hint_tag_evidence(t, action)[1]) for t in targets])
    if legacy_warn:
        print(f"[hint_tag] ⚠ v1 빈티지 {len(legacy_warn)}개는 evidence-binding footer 이전 태그다 "
              f"(계약 §4 — 재작성 ✗ · SHA 핀 일치 확인됨). 경고로만 통과시킨다.", file=sys.stderr)
    if drift_warn:
        print(f"[hint_tag] ⚠ 증거-보전 드리프트 {len(drift_warn)}개 — 발행 후 매니페스트가 승인된 편집으로 "
              f"바뀌었고 identity·certificate·판정은 보존됐다(등재: hints/evidence_drift_pins.json). "
              f"경고로만 통과시킨다.", file=sys.stderr)
    # 배포 평면도 발행자 평면이다 — 부재/미봉인은 여기서 차단한다(사용자 결정 α의 경계).
    blocking = list(blocking)
    blocking += [f"{x}: HINT_EVIDENCE_REF_ABSENT 참조 증거가 이 체크아웃에 없다 — "
                 "배포하려면 증거를 보유해야 한다" for x in unverifiable]
    blocking += [f"{x}: HINT_TAG_UNSEALED {_r}" for x in targets if (_r := unsealed_reason(x))]
    if blocking:
        _die_binding(action, ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "one or more hint tags carry forged/drifted evidence, are unsealed (not a "
                      "`seal` product), or a v2-era tag lacks its binding -- refusing (no partial "
                      "rewrite/push):\n  " + "\n  ".join(blocking)},
                     None, None)


# ── push (선별) ──────────────────────────────────────────────────────────────
def cmd_push(a: argparse.Namespace) -> int:
    _require_central("push")
    _require_promotion_authorization("push", a.manifest)
    # 검증 범위 = 배포 범위. `--tag` 를 주면 그 패턴에 맞는 태그만 검증하고 그것만 민다.
    # 완화가 아니라 정밀화다 — 계약 §2 의 위협은 태그별 속성이라 A 의 드리프트가 B 의 주장을
    # 거짓으로 만들지 않는다. 전수 검증은 위협 모델이 아니라 `refs/tags/hint/*` 라는 refspec
    # 선택에서 따라온 결합이었고, 이미 원격에 있어 재-push 가 no-op 인 태그가 신규 발행을 영구히
    # 막았다(2026-08-20 실증: 드리프트 16종이 hy3 7종을 가로막음).
    selected = None
    if getattr(a, "tag", None):
        # 패턴은 반드시 hint 네임스페이스 안이어야 한다. 이걸 안 걸면 `--tag '*'` 가
        # refspec `refs/tags/*` 로 번역돼 **로컬 last-good-* 롤백 태그가 공개 origin 으로 샌다**
        # (계약 C5 가 막는 바로 그 사고 · 2026-08-20 이 가드가 실제로 내 결함을 잡았다).
        if not a.tag.startswith("hint/"):
            die(f"[hint_tag] FAIL: --tag 은 'hint/' 로 시작해야 한다: {a.tag!r} — "
                "refspec 이 hint 네임스페이스를 벗어나면 last-good-* 가 유출된다(계약 C5).")
        selected = [t for t in existing_hint_tags() if fnmatch.fnmatch(t, a.tag)]
        if not selected:
            die(f"[hint_tag] FAIL: --tag 패턴에 맞는 로컬 hint 태그 없음: {a.tag!r}")
    _require_all_hint_tags_evidence_valid("hint_push", selected)
    refspec = "refs/tags/hint/*"
    if selected:
        refspec = f"refs/tags/{a.tag}"
    if selected:
        print(f"[hint_tag] 선별 배포 대상 {len(selected)}종 (검증 완료):")
        for t in selected:
            print(f"             {t}")
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
    """근-미스 발견. **모델 비교는 정규화 + family 해소 후**에 한다 — 2026-08-20 실측에서
    `--model gemma-4-e2b-it` 검색이 정확일치인 `gemma-4-E2B-it/gb10` 을 '다른 모델'로 판정했다.
    그리고 모델 불일치분은 기본 숨긴다(출력 9,563 B 중 46/49 가 잡음이었다)."""
    idx, fam = _load_index(), _load_families()
    _, slugs = resolve_family(a.model, fam)
    nslugs = {_norm_slug(s) for s in slugs} | {_norm_slug(canonicalize(a.model) or a.model)}
    scored = []
    for e in idx["hints"]:
        d = (e["vllm"] == a.vllm, _norm_slug(e["model"]) in nslugs, e["arch"] == a.arch)
        scored.append((sum(d), e, d))
    scored.sort(key=lambda x: (-x[0], _vkey(x[1]["vllm"])))
    if not scored:
        print("[hint_tag] (인덱스 비어있음)")
        return 0
    shown = 0
    for score, e, d in scored:
        if not d[1] and not a.include_other:
            continue
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
        shown += 1
    hidden = len(scored) - shown
    if not shown:
        print(f"[hint_tag] 같은 모델(family) 태그 없음. 다른 모델 {hidden}종은 --include-other 로.")
    elif hidden:
        print(f"[hint_tag] (다른 모델 {hidden}종 숨김 — --include-other · 전 이력은 `collect --model {a.model}`)")
    return 0


# ── D8 권한 비대칭 (plan_26082009 §4) ────────────────────────────────────────
# 사용자 제약: "HINTS.md 의 관리 주체는 단 1종으로 한정". 발행(seal)은 분산, 색인·배포는 중앙집중이다.
# 마커는 **gitignored** 라 배포본에 실리지 않는다 — 즉 배포받은 Contributor 환경에서는 존재할 수
# 없고, 게이트가 fail-closed 로 닫힌다. 신원 체계 없이 결정론으로 집행하는 가장 단순한 수단이다.
CENTRAL_MARKER = ROOT / "hints" / ".central_authority"


def _require_central(action: str) -> None:
    if CENTRAL_MARKER.is_file():
        return
    # 안내가 막다른 길로 읽히면 사람도 에이전트도 우회를 택한다(workflow.md D5 · plan_26082017 W7).
    # 종전 문구는 "중앙 전용이다"에서 끝나 **여는 법**을 말하지 않았고, 그 결과 배포받은 프로젝트가
    # 맨 `git tag -a` + `git push` 로 돌아갔다(2026-08-20 실증). 처방을 함께 적는다.
    die(f"[hint_tag] FAIL: '{action}' 는 **자기 원격의 색인·배포 권위**를 가진 체크아웃에서만 실행된다 "
        f"— `hints/.central_authority` 부재.\n"
        f"        ▸ 이 체크아웃이 **자기 원격**(origin)의 hint 카탈로그를 소유한다면 권위를 선언하라:\n"
        f"            printf '%s\\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' "
        f"> hints/.central_authority\n"
        f"          (비추적이다 — 배포본에 실리지 않으므로 **각 저장소가 스스로** 선언해야 한다.\n"
        f"           선언은 권한이자 책임이다: 그 원격의 index.json·HINTS.md 정합을 떠안는다.)\n"
        f"        ▸ 이 체크아웃이 **상류에 기여**하는 입장이라면 `seal` 로 로컬 태그까지만 만들고\n"
        f"          그 태그를 상류에 전달하라. 색인·배포는 상류가 한다.\n"
        f"        ▸ 어느 쪽도 아니면 **우회하지 말고** 어느 쪽인지부터 정하라 — 맨 `git tag -a` +\n"
        f"          `git push` 로 만든 태그는 증거 바인딩이 없어 수신자에게 `unbound` 로 격리된다.\n"
        f"        (근거: plan_26082009 D8 · plan_26082017 §4.2 W2-a)")


def cmd_index(a: argparse.Namespace) -> int:
    """이미 로컬에 존재하는 hint 태그를 색인에 편입한다(중앙 전용).
    `seal` 이 만든 태그를 받아 여기서 index.json + HINTS.md 를 갱신한다."""
    _require_central("index")
    tags = existing_hint_tags()
    if a.tag not in tags:
        die(f"[hint_tag] FAIL: 로컬에 없는 태그: {a.tag}")
    # `-p` 는 오브젝트 **전문**(object/type/tag/tagger 헤더 포함)을 준다 — 그걸 body 로 넘기면
    # _brief_of 가 첫 줄 `object <sha>` 를 brief 로 집는다(2026-08-20 실측 7/48 파손).
    # 헤더/본문 분리는 `_parse_tag_body`·PII 스캔이 쓰는 관용구를 그대로 재사용한다.
    typ = git("cat-file", "-t", a.tag).stdout.strip()
    if typ != "tag":
        die(f"[hint_tag] FAIL: annotated 태그가 아니다(type={typ}): {a.tag}\n"
            f"        hint 태그는 `seal` 이 만든 annotated 태그여야 한다(본문이 곧 페이로드다).")
    _, _, body = git("cat-file", "tag", a.tag).stdout.partition("\n\n")
    _, vllm, model, arch = a.tag.split("/")
    fm = dict(re.findall(r"^(\w+):\s*(.+)$", body, re.M))
    topology = a.topology or fm.get("topology", "")
    anchor = fm.get("anchor") or git("rev-list", "-n", "1", a.tag).stdout.strip()
    if not topology:
        die("[hint_tag] FAIL: 태그 footer 에 topology 가 없고 --topology 도 미지정.")
    idx = _load_index()
    idx["hints"] = [e for e in idx["hints"] if e["tag"] != a.tag]
    idx["hints"].append({
        "tag": a.tag, "vllm": vllm, "model": model, "arch": arch,
        "topology": topology, "brief": _brief_of(body), "anchor": anchor,
        "related": a.related or "", "status": "active",
        "last_verified": date.today().isoformat(),
    })
    idx["hints"].sort(key=lambda e: e["tag"])
    _save_index(idx)
    _hints_regen(idx["hints"])
    print(f"[hint_tag] index.json + HINTS.md 갱신: {a.tag}")
    return 0

# ── R1 슬러그 파생 + D10 린터 (plan_26082008 R1 · plan_26082009 §6) ──────────
def derive_slug(hf_repo: str | None, model_path: str | None) -> tuple[str, str]:
    """→ (slug, source). **발행자가 이름을 짓지 못하게** 한다.

    2026-08-20 실측: 발행된 32 슬러그 중 27종이 정본표 밖이었고(`--allow-new-slug` 통과),
    같은 모델이 철자로 2건 갈라졌다(`gemma-4-e2b-it`↔`-E2B-it` · `qwen3.5-…`↔`qwen35-…`).
    원인은 **작명 자유도**이므로 처방은 자유도 제거다 — 슬러그는 HF repo 이름에서 파생한다(사용자 D2).
    HF 미등록 커스텀 모델만 예외이며 **서빙에 사용한 경로**로 명명한다(D2 예외)."""
    if hf_repo:
        if "/" not in hf_repo:
            die(f"[hint_tag] FAIL: --hf-repo 는 '<org>/<name>' 형태여야 함: {hf_repo}")
        return hf_repo.rstrip("/").split("/")[-1].lower(), "hf_repo"
    if model_path:
        base = os.path.basename(model_path.rstrip("/"))
        if not base:
            die(f"[hint_tag] FAIL: --model-path 에서 이름을 못 뽑음: {model_path}")
        return re.sub(r"[^a-z0-9._-]+", "-", base.lower()).strip("-"), "model_path"
    die("[hint_tag] FAIL: --hf-repo(정본) 또는 --model-path(HF 미등록 커스텀) 중 하나가 필요함. "
        "슬러그는 발행자가 짓지 않는다(plan_26082008 R1).")
    return "", ""            # unreachable — die() 는 SystemExit


def require_derived_slug(tag_model: str, vllm: str, arch: str,
                         hf_repo: str | None, model_path: str | None) -> str:
    """태그의 `<model>` 슬롯이 파생 슬러그와 같은지 검사. 다르면 **정확한 태그 이름을 알려주고 죽는다**.
    연속성 예외: 이미 등재된 같은 family 의 정본 철자면 통과한다 — 새 철자를 만들 수는 없고
    (허용 목록이 추적 색인에서 온다) 기존 계보를 잇는 것만 된다."""
    slug, src = derive_slug(hf_repo, model_path)
    if tag_model == slug:
        return src
    fid, slugs = resolve_family(slug, _load_families())
    if fid is not None and tag_model in slugs:
        return src + "+family-continuity"
    die(f"[hint_tag] FAIL: 태그의 model 슬롯 '{tag_model}' 가 파생 슬러그 '{slug}' 와 다름.\n"
        f"        올바른 이름: hint/{vllm}/{slug}/{arch}\n"
        f"        (연속성 예외는 `hints/families.json` 에 등재된 같은 family 슬러그만 해당)")
    return ""


# L1 필수 절. **템플릿에만 있고 산출물에는 없던 구조**를 여기서 강제한다 —
# 2026-08-20 실측: 발행된 49/49 태그에 `#` 로 시작하는 라인이 0개였고 밀도가 12배 벌어졌다.
# 템플릿이 스킬 안에 있어도 아무것도 막지 못했다. 막는 코드가 없었기 때문이다.
REQUIRED_SECTIONS = [
    (1, "벽 지도"), (2, "결정론 해소값"), (3, "모델 서빙 노브"),
    (4, "빌드평면 노브"), (5, "성능 baseline"), (6, "재검증"), (7, "메타"),
]
FREE_SECTION = 8                        # `## 8. comment` — 자유 기술 칸(검사 제외)
MIN_SECTION_CHARS = 80                  # L2: 공백 제외
TRANSFER_CLASS = re.compile(r"arch-(?:invariant|scaled|locked|LOCKED)")
GENERIC_ONLY = re.compile(r"^[\s\-*·>]*(재검증하라|재확인하라|주의하라|참고하라)[\s.·]*$", re.M)


def _split_sections(body: str) -> dict[int, str]:
    out, cur, buf = {}, None, []
    for line in body.split("\n"):
        m = re.match(r"^##\s*(\d+)\.", line)
        if m:
            if cur is not None:
                out[cur] = "\n".join(buf)
            cur, buf = int(m.group(1)), []
        elif cur is not None:
            buf.append(line)
    if cur is not None:
        out[cur] = "\n".join(buf)
    return out


def lint_body(body: str) -> list[str]:
    """D10 L1–L5. fail-closed 로 쓰인다 — 통과 못 하면 태그가 안 나간다.

    이것이 **일관성의 실제 보장**이다(스킬 승격이 아니라). 실증: 지금까지 집행된 검사는
    `TODO(judgment` 한 줄뿐이었고 **그 항목만 100% 지켜졌다**(plan_26082009 §1)."""
    problems: list[str] = []
    secs = _split_sections(body)
    for n, name in REQUIRED_SECTIONS:                                   # L1
        if n not in secs:
            problems.append(f"L1 필수 절 누락: `## {n}. {name}`")
    for n, name in REQUIRED_SECTIONS:                                   # L2
        if n in secs and len(re.sub(r"\s", "", secs[n])) < MIN_SECTION_CHARS:
            problems.append(f"L2 절 `{n}. {name}` 밀도 부족(공백제외 "
                            f"{len(re.sub(chr(92) + 's', '', secs[n]))} < {MIN_SECTION_CHARS}자)")
    if not TRANSFER_CLASS.search(body):                                 # L3
        problems.append("L3 전이등급 태깅(arch-invariant/scaled/locked) 이 한 번도 없음 — "
                        "수신자가 무엇을 복사하면 안 되는지 알 수 없다")
    if "TODO(judgment" in body:                                         # L4 (기존)
        problems.append("L4 TODO(judgment) 슬롯 잔존 — 에이전트가 저작해야 함")
    for n, name in REQUIRED_SECTIONS:                                   # L5
        if n in secs and GENERIC_ONLY.search(secs[n]):
            problems.append(f"L5 절 `{n}. {name}` 이 generic 문구뿐 — 이 HW/모델 특정 사실을 적어라")
    return problems

# ── family 색인 (plan_26082008 R2·R5·R6 · plan_26082009 D9) ───────────────────
# family 는 `hints/families.json` 에 산다 — `reindex` 가 index.json 을 태그에서 **재생성**하므로
# 거기 두면 재생성이 파괴한다. 생성/감사는 `bootstrap_families.py` 가 소유한다.
FAMILIES_FILE = ROOT / "hints" / "families.json"


def _norm_slug(s: str) -> str:
    """슬러그 비교용 정규화. 2026-08-20 실측: 대소문자·구두점만 다른 동일 모델이 이미 2건
    갈라져 있었고(`gemma-4-e2b-it`↔`gemma-4-E2B-it` · `qwen3.5-…`↔`qwen35-…`), `match` 가
    정확일치 태그를 '다른 모델'로 판정했다. 비교는 반드시 정규화 후에 한다."""
    return re.sub(r"[^a-z0-9]", "", s.lower())


def _load_families() -> dict:
    if not FAMILIES_FILE.is_file():
        return {"families": {}, "tag_sd": {}}
    return json.loads(FAMILIES_FILE.read_text(encoding="utf-8"))


def resolve_family(model: str, fam: dict) -> tuple[str | None, set[str]]:
    """model(슬러그·family id·철자변형 무엇이든) → (family_id, 소속 슬러그 집합).
    미등재면 (None, {정규화 동치 슬러그들}) 로 떨어져 **최소한 철자 갈림은 흡수한다**."""
    n = _norm_slug(model)
    fams = fam.get("families", {})
    for fid, f in fams.items():
        if (_norm_slug(fid) == n
                or any(_norm_slug(m["slug"]) == n for m in f["members"])
                # 서빙 repo 이름으로도 이어붙인다 — 태그 슬러그와 repo 가 다를 수 있다
                # (예: 태그 `hy3` ↔ repo `Hy3-NVFP4-W4A16`). 사용자 D2 의 "모델별 출처 기재" 가
                # 여기서 실제로 쓰인다.
                or any(m.get("repo") and _norm_slug(m["repo"]) == n for m in f["members"])):
            return fid, {m["slug"] for m in f["members"]}
    return None, {model}


def _vkey(v: str) -> tuple:
    return tuple(int(x) if x.isdigit() else 0 for x in re.split(r"[._-]", v)[:4])


def _sd_brief(sd: dict | None) -> str:
    if not sd:
        return "-"
    if sd.get("enabled") is None:
        return "?"
    if not sd["enabled"]:
        return "off"
    bits = []
    if sd.get("spec_tokens") is not None:
        bits.append(f"spec={sd['spec_tokens']}")
    if sd.get("accept_len") is not None:
        bits.append(f"acc={sd['accept_len']}")
    return "on" + ("(" + "·".join(bits) + ")" if bits else "")


def cmd_collect(a: argparse.Namespace) -> int:
    """수신자용 **이력 수집**. index+families 만 읽는다 — git 오브젝트를 열지 않으므로 결정론이고
    토큰이 적다(현행 `match` 는 49종을 9.5 KB 로 쏟았다).

    관계 판정은 **하지 않는다**: 무엇이 패턴이고 무엇이 안티패턴인지는 시간축을 봐야 알고,
    그건 수신자만 볼 수 있다(사용자 D3). 여기서는 **빠짐없는 수집**만 보장한다."""
    idx, fam = _load_index(), _load_families()
    fid, slugs = resolve_family(a.model, fam)
    nslugs = {_norm_slug(s) for s in slugs}
    rows = [e for e in idx["hints"] if _norm_slug(e["model"]) in nslugs]
    if a.sd_only:
        rows = [e for e in rows if (fam.get("tag_sd", {}).get(e["tag"], {}) or {}).get("enabled")]
    rows.sort(key=lambda e: (_vkey(e["vllm"]), e["arch"]))
    if a.json:
        print(json.dumps({"family": fid, "slugs": sorted(slugs),
                          "sd_capability": (fam.get("families", {}).get(fid) or {}).get("sd_capability"),
                          "hints": [dict(e, sd=fam.get("tag_sd", {}).get(e["tag"])) for e in rows]},
                         ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    if fid is None:
        print(f"[collect] family 미등재: '{a.model}' — 철자 동치만으로 수집했다.")
        print("          `bootstrap_families.py --write` 로 등재하면 양자화·변종·SD 초안까지 묶인다.")
    else:
        f = fam["families"][fid]
        mem = " · ".join(f"{m['slug']}({m['relation']})" for m in f["members"])
        print(f"[collect] family `{fid}`" + (f"  root={f['root_repo']}" if f.get("root_repo") else ""))
        print(f"          멤버: {mem}")
        cap = f.get("sd_capability") or {}
        print(f"          SD 능력: {cap.get('mode','unknown')}  [{cap.get('source','-')}]")
    if not rows:
        print("[collect] 해당 태그 없음.")
        return 0
    print(f"\n  {'vLLM':<8} {'arch':<26} {'SD':<18} tag")
    for e in rows:
        sd = _sd_brief(fam.get("tag_sd", {}).get(e["tag"]))
        mark = {"unbound": "  ⚠unbound", "unverifiable": "  ⓘ대조불가"}.get(e.get("status"), "")
        print(f"  {e['vllm']:<8} {e['arch']:<26} {sd:<18} {e['tag']}{mark}")
    # 격리분을 조용히 섞어 내보내면 수신자가 근거로 쓴다(plan_26082017 R2). 소리 내어 구분한다.
    unv = [e for e in rows if e.get("status") == "unverifiable"]
    if unv:
        print(f"\n  ⓘ 위 {len(unv)}종은 **대조 불가(unverifiable)** 다 — 증거 파일이 배포되지 않아"
              "\n    이 클론에서 digest 를 맞춰볼 수 없을 뿐, 결함이라는 뜻은 아니다. 평소대로 쓰되"
              "\n    **네 환경에서 재검증**하라(원래도 그게 규칙이다).")
    unb = [e for e in rows if e.get("status") == "unbound"]
    if unb:
        print(f"\n  ⚠ 위 {len(unb)}종은 **증거 바인딩 불량(unbound)** 이다 — 발행 도구를 거치지 않았거나"
              "\n    바인딩이 해소되지 않는다. **서빙전략 근거로 쓰지 마라**(수치의 출처를 확인할 수 없다).")
        for e in unb:
            print(f"      - {e['tag']}: {e.get('unbound_reason', '(사유 미기록)')}")
    print(f"\n  총 {len(rows)}종 — **시간축이다**: 뒤 항목이 앞 항목을 안티패턴으로 만들 수 있다.")
    print("  판정은 네가 한다(발행자는 관계를 적지 않는다). 본문:")
    print("     git tag -l --format='%(contents)' <tag> > seed/hints/<name>.md")
    return 0

# ── orphans (수집 가능성 대사 · read-only · ungated) ─────────────────────────
def cmd_orphans(a: argparse.Namespace) -> int:
    """태그 ↔ index.json ↔ families.json 3중 대사. **수집 가능성의 단일 권위**다.

    `collect` 는 결정론을 위해 index+families 만 읽는다(git 오브젝트를 열지 않는다). 그래서 거기
    없는 태그는 **침묵 누락**된다 — 수신자 로컬에 오브젝트가 있어도 존재하지 않는 것과 같다.
    2026-08-20 실증: `hint/0.27.1/*` 2건이 원격에 있고 fetch 도 됐는데 `collect` 에 안 잡혔다.
    이 명령은 그 침묵을 소리로 바꾼다 — 발행자가 계약 §1 의 유일한 의무(**빠짐없는 수집**)를
    지켰는지 확인하는 자리다.

    슬러그 대조는 `resolve_family` 와 같은 `_norm_slug` 를 쓴다(철자 갈림으로 인한 위양성 차단).
    """
    tags = set(existing_hint_tags())
    idx, fam = _load_index(), _load_families()
    indexed = {e["tag"] for e in idx.get("hints", [])}
    fam_members = {m["slug"] for f in fam.get("families", {}).values() for m in f.get("members", [])}
    fam_norm = {_norm_slug(s) for s in fam_members}
    tag_slug_of = {}
    for tg in tags:
        tag_slug_of.setdefault(tg.split("/")[2], []).append(tg)

    findings: list[tuple[str, list[str]]] = [
        ("태그에 있는데 색인에 없음 — `collect` 에 안 잡힌다", sorted(tags - indexed)),
        ("색인에 있는데 로컬 태그가 없음 — 미배포이거나 fetch 안 됨", sorted(indexed - tags)),
        ("태그 슬러그가 family 미등재 — family 해소가 안 된다",
         sorted(s for s in tag_slug_of if _norm_slug(s) not in fam_norm)),
        ("family 멤버인데 그 슬러그의 태그가 없음",
         sorted(s for s in fam_members
                if _norm_slug(s) not in {_norm_slug(x) for x in tag_slug_of})),
    ]
    if a.remote:
        ls = git("ls-remote", "--tags", a.remote, check=False)
        if ls.returncode != 0:
            print(f"  ⚠ 원격 대사 생략 — `git ls-remote {a.remote}` 실패(네트워크/권한). "
                  "로컬 대사만 보고한다.", file=sys.stderr)
        else:
            remote = {ln.split("refs/tags/", 1)[1] for ln in ls.stdout.splitlines()
                      if "refs/tags/hint/" in ln and not ln.rstrip().endswith("^{}")}
            findings += [
                (f"원격({a.remote})에만 있음 — fetch 하면 보인다", sorted(remote - tags)),
                (f"로컬에만 있음 — 아직 {a.remote} 로 push 되지 않았다", sorted(tags - remote)),
            ]

    print(f"[orphans] 로컬 태그 {len(tags)} · 색인 {len(indexed)} · family 슬러그 {len(fam_members)}")
    total = 0
    for title, items in findings:
        if not items:
            continue
        total += len(items)
        print(f"\n  ✗ {title} ({len(items)})")
        for x in items:
            print(f"     - {x}")
    if total == 0:
        print("\n  ✓ 대사 일치 — 모든 태그가 수집 가능하다.")
        return 0
    print(f"\n  총 {total} 건 — 색인 편입은 `index --tag <tag> [--topology …]`, "
          "family 등재는 `bootstrap_families.py --write` 다.")
    return 1


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
    # 단일 대상이지만 분류는 동일 권위를 쓴다 — v1 빈티지 태그의 currency 스탬프 갱신까지
    # 막을 이유가 없다(계약 §4). forged/drifted 는 여기서도 그대로 차단된다.
    _blocking, _legacy, _drift, _unver = classify_evidence_problems([(a.tag, ev_problems)])
    if _legacy:
        print(f"[hint_tag] ⚠ {a.tag} 는 v1 빈티지(footer 이전 · SHA 핀 일치) — 경고로 통과.",
              file=sys.stderr)
    if _blocking:
        _die_binding("hint_reverify", ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "target tag's evidence is forged/drifted, or a v2-era tag lacks its binding:\n  "
                      + "\n  ".join(_blocking)},
                     manifest.get("identity"), manifest.get("task_class"))
    reachable = git("cat-file", "-e", entry["anchor"] + "^{commit}", check=False).returncode == 0
    entry["last_verified"] = date.today().isoformat()
    entry["anchor_reachable"] = reachable
    _save_index(idx)
    print(f"[hint_tag] reverify {a.tag}: anchor_reachable={reachable} · last_verified 스탬프.")
    return 0


# ── reindex (태그 = 진실원천 → index.json + HINTS.md 재생성) ──────────────────
def cmd_reindex(a: argparse.Namespace) -> int:
    _require_central("reindex")
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
    per_tag: list = []
    for t in tags:
        footer, ev_problems = _validate_hint_tag_evidence(t, "hint_reindex")
        if footer is not None:
            footers[t] = footer
        per_tag.append((t, ev_problems))
    # 분류는 classify_evidence_problems 단일 권위(계약 v2 §5) — 여기서 복제하지 않는다.
    problems, legacy_warn, drift_warn, unverifiable = classify_evidence_problems(per_tag)
    if legacy_warn:
        print(f"[hint_tag] ⚠ v1 빈티지 {len(legacy_warn)}개는 footer 이전 태그다(계약 §4 · SHA 핀 일치). "
              "인덱스에는 포함하되 경고로만 통과시킨다.", file=sys.stderr)
    # 태그별 귀속 — 같은 단일 권위(classify_evidence_problems)를 태그 하나씩 다시 태운다.
    # 메시지 문자열을 파싱해 태그를 캐내지 않는다(포맷이 바뀌면 조용히 어긋난다).
    unbound_why: dict[str, str] = {}
    unver_why: dict[str, str] = {}
    for _t, _ev in per_tag:
        _b, _l, _d, _u = classify_evidence_problems([(_t, _ev)])
        _uns = unsealed_reason(_t)                      # 파일 불요 — 수신자 평면에서도 작동한다
        if _b or _uns:
            # 사유 문자열에는 태그를 넣지 않는다 — 소비처(collect)가 이미 태그를 찍으므로
            # 그대로 두면 이름이 두 번 나온다.
            _why = _b[0] if _b else f"HINT_TAG_UNSEALED {_uns}"
            unbound_why[_t] = _why[len(_t) + 2:] if _why.startswith(f"{_t}: ") else _why
        elif _u:
            unver_why[_t] = ("참조 증거가 이 체크아웃에 없다 — 여기서는 대조 불가"
                             "(증거는 배포되지 않는다. 발행자 평면에서 확인해야 한다)")
    if problems and a.strict:
        _die_binding("hint_reindex", ["HINT_EVIDENCE_BINDING_INCOMPLETE"],
                     {"HINT_EVIDENCE_BINDING_INCOMPLETE":
                      "one or more hint tags carry forged/drifted evidence, or a v2-era tag lacks "
                      "its binding -- refusing (no partial rewrite):\n  " + "\n  ".join(problems)
                      + _legacy_remedy_hint(problems)},
                     None, None)
    if unbound_why:
        # 격리(plan_26082017 §4.3 (a) · 2026-08-20 사용자 결정): 전량 차단은 무결한 나머지의
        # 갱신까지 인질로 잡는다. 대신 **침묵 배제 금지** — 여기서 열거하고, index 에
        # status="unbound" 로 박고, collect 가 경고와 함께 보여준다.
        print(f"[hint_tag] ⚠ 증거 바인딩 불량 {len(unbound_why)}건을 **격리**한다 "
              f"(status=unbound · 색인에는 남지만 근거로 쓰지 마라):", file=sys.stderr)
        for _t in sorted(unbound_why):
            print(f"    - {_t}\n        {unbound_why[_t]}", file=sys.stderr)
        print("    → 전량 차단을 원하면 `reindex --strict`.", file=sys.stderr)
    if unver_why:
        print(f"[hint_tag] ⓘ 대조 불가 {len(unver_why)}건 (status=unverifiable · **차단 아님**) — "
              "증거가 이 체크아웃에 없을 뿐이다.", file=sys.stderr)
    idx = _load_index()
    prev = {e["tag"]: e for e in idx["hints"]}
    hints = []
    for t in tags:
        _, vllm, model, arch = t.split("/")
        # v1 빈티지는 footer 가 **없다**(계약 §4 — 재작성 금지). 종전엔 게이트가 footer 를
        # 보장했기에 footers[t] 를 무조건 인덱싱했고, v2 에서 레거시가 통과하게 되자
        # KeyError 로 죽었다. footer 부재 시 기존 인덱스 항목(prev)에서 승계한다 —
        # 그 값들은 v1 시절 finalize 가 기록해둔 것이라 날조가 아니다.
        # 격리 대상은 **footer 를 신뢰하지 않는다** — 바인딩이 불량이라고 판정한 그 footer 에서
        # topology/anchor 를 다시 읽으면 불량분을 정본으로 승격시키는 셈이다. 태그 오브젝트
        # 자체(본문·rev-list)에서만 재구성한다.
        is_unbound = t in unbound_why
        is_unver = t in unver_why
        footer = None if is_unbound else footers.get(t)
        brief, _old_topology, related = _parse_tag_body(t)
        p = prev.get(t, {})
        e = {
            "tag": t, "vllm": vllm, "model": model, "arch": arch,
            "topology": (footer or {}).get("topology") or p.get("topology") or _old_topology or "",
            "brief": brief or p.get("brief", ""),
            "anchor": ((footer or {}).get("anchor") or p.get("anchor")
                       or git("rev-list", "-n", "1", t).stdout.strip()),
            "related": p.get("related") or related,  # 큐레이트(finalize) 우선, 없으면 본문 파싱
            "status": ("unbound" if is_unbound else
                       "unverifiable" if is_unver else p.get("status", "active")),
            "last_verified": p.get("last_verified", date.today().isoformat()),
        }
        if is_unbound:
            e["unbound_reason"] = unbound_why[t]
        elif is_unver:
            e["unbound_reason"] = unver_why[t]
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
    c.add_argument("--hf-repo", help="정본 HF repo '<org>/<name>' — 슬러그가 여기서 파생된다(R1)")
    c.add_argument("--model-path", help="HF 미등록 커스텀 모델의 서빙 경로(D2 예외)")
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
    f.add_argument("--hf-repo", help="정본 HF repo '<org>/<name>' — 슬러그가 여기서 파생된다(R1)")
    f.add_argument("--model-path", help="HF 미등록 커스텀 모델의 서빙 경로(D2 예외)")
    f.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    f.set_defaults(fn=cmd_finalize)

    # `seal` = finalize 에서 **색인 갱신만 뺀 것**. Contributor 의 종착점이다(D8).
    sl = sub.add_parser("seal", help="PII 스캔 + 린트 + annotated 태그 생성까지 (색인 ✗ · Contributor 용)")
    for _a in ("--tag", "--recipe", "--topology", "--manifest"):
        sl.add_argument(_a, required=True)
    sl.add_argument("--commit", default="HEAD")
    sl.add_argument("--related", default="")
    sl.add_argument("--tagger-name", default="")
    sl.add_argument("--tagger-email", default="")
    sl.add_argument("--allow-new-slug", action="store_true")
    sl.add_argument("--hf-repo", help="정본 HF repo '<org>/<name>' — 슬러그가 여기서 파생된다(R1)")
    sl.add_argument("--model-path", help="HF 미등록 커스텀 모델의 서빙 경로(D2 예외)")
    sl.set_defaults(fn=cmd_finalize, no_index=True)

    ix = sub.add_parser("index", help="로컬 hint 태그를 index.json + HINTS.md 에 편입 (중앙 전용)")
    ix.add_argument("--tag", required=True)
    ix.add_argument("--topology", default="", help="미지정 시 태그 footer 에서 읽는다")
    ix.add_argument("--related", default="")
    ix.set_defaults(fn=cmd_index)

    v = sub.add_parser("verify", help="릴리즈 게이트(태그오브젝트/build_patches PII·인덱스 정합)")
    v.add_argument("--check-origin", action="store_true", help="origin 에 last-good-* 없음 확인(네트워크)")
    v.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    v.set_defaults(fn=cmd_verify)

    p = sub.add_parser("push", help="선별 배포 refs/tags/hint/* (--tags 금지)")
    p.add_argument("--remote", default="origin")
    p.add_argument("--apply", action="store_true")
    p.add_argument("--tag", default="",
                   help="배포할 태그 glob(예: 'hint/0.27.0/hy3/*'). 지정 시 **그 태그만** 검증하고 "
                        "그것만 민다 — 검증 범위를 배포 범위에 맞춘다. 미지정 시 전 hint 태그.")
    p.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    p.set_defaults(fn=cmd_push)

    m = sub.add_parser("match", help="근-미스 발견(축별 이식 가이드) -- read-only, ungated")
    m.add_argument("--vllm", required=True)
    m.add_argument("--model", required=True)
    m.add_argument("--arch", required=True)
    m.set_defaults(fn=cmd_match)
    m.add_argument("--include-other", action="store_true",
                   help="다른 모델 태그까지 나열(기본 숨김 — 2026-08-20 실측 9,563 B 중 46종이 잡음이었다)")

    co = sub.add_parser("collect", help="한 모델의 **전 이력**을 시간순 수집(family 해소) -- read-only, ungated")
    co.add_argument("--model", required=True, help="슬러그·family id·철자변형 무엇이든")
    co.add_argument("--sd-only", action="store_true", help="SD(speculative decoding)를 실제로 켠 레시피만")
    co.add_argument("--json", action="store_true", help="기계판독 출력")
    co.set_defaults(fn=cmd_collect)

    orp = sub.add_parser("orphans",
                         help="태그↔index↔families 3중 대사(수집 가능성) -- read-only, ungated")
    orp.add_argument("--remote", nargs="?", const="origin", default=None,
                     help="원격까지 대사(기본 origin). 네트워크를 쓴다")
    orp.set_defaults(fn=cmd_orphans)

    r = sub.add_parser("reverify", help="핀 자산 reachability + last_verified 스탬프")
    r.add_argument("--tag", required=True)
    r.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    r.set_defaults(fn=cmd_reverify)

    ri = sub.add_parser("reindex", help="전 hint 태그에서 index.json+HINTS.md 재생성(브랜치 드리프트 정합·currency 보존)")
    ri.add_argument("--strict", action="store_true",
                    help="증거 바인딩 불량이 하나라도 있으면 전량 차단(옛 동작). 기본은 격리+경고")
    ri.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    ri.set_defaults(fn=cmd_reindex)

    pl = sub.add_parser("pin-legacy",
                        help="v2 발효 이전 발행 + footer 부재 태그를 legacy_v1_pins 에 SHA 등재(피어 호스트 태그 뒤늦은 페치 대응)")
    pl.add_argument("--tag", required=True)
    pl.add_argument("--manifest", required=True, help="promotion-ready work-manifest (completion_gate.py authorize --mode promotion)")
    pl.set_defaults(fn=cmd_pin_legacy)

    a = ap.parse_args()
    if a.cmd != "match":
        require_git_repository()
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
