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
import json
import os
import re
import shlex
import subprocess
import sys
from datetime import date, datetime
from pathlib import Path

# 배포 태그 오브젝트에 박히는 tagger 신원의 기본값. **발행자 개인 신원이 아니라 프로젝트
# 합성 신원**이며, 발행된 62개 태그가 이미 이 값을 쓰고 있다(관행의 코드화). `.invalid` 는
# RFC2606 예약 TLD 라 실제로 도달하지 않는다.
DEFAULT_TAGGER_NAME = "easy-vllm-simulator"
DEFAULT_TAGGER_EMAIL = "hints@easy-vllm.invalid"

# ★ 2026-09-04(CP7 · plan_26090415 §7.5 M1): 태그가 **5세그먼트**가 된다.
#     hint/<vllm>/<model>/<arch>/<recipe>
#   왜: 이름에 레시피 축이 없어서 한 스윕의 여러 셀이 **같은 이름**을 원했고, 그때 이 스크립트는
#   기존 이름을 하드 차단한다(발행 불가). 이미 벌어져 있던 일이다 — 같은 모델·다른 max_model_len
#   인증서가 실재하고 레시피 변이를 가진 모델이 6종이다. `serving_config` 는 축으로 못 쓴다
#   (45건 중 12건만 채워졌고 그마저 모델명으로 덮인다).
#   레시피를 **맨 뒤**에 붙이는 이유: 모델 슬러그 인덱스(`split("/")[2]`)가 그대로 살아 변경
#   표면이 가장 작다.
TAG_SHAPE = re.compile(r"^hint/[^/]+/[^/]+/[^/]+/[^/]+$")
# 레시피 세그먼트는 **파생값**이다(손저작 ✗). 형태를 좁혀 두면 손으로 지은 이름이 여기서 걸린다.
RECIPE_SHAPE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")

# ★ 2026-09-06(plan_26090616 Q1/Q2 · 사용자 결정): arch 세그먼트에 **노드 축**이 들어간다.
#     <hw>-<main|sub|cluster>-<target>      예) gb10-main-sim-h100 · gb10x2-cluster-sim-h100
#   왜: hint 태그는 "이 조합이 된다"가 아니라 **"어느 노드 형상이 수행한 기록"** 이다. 옛 문법
#   `gb10-sim-h100` 에는 수행 주체가 없어서, 같은 하드웨어의 메인·서브가 같은 이름을 원했고 —
#   실제로 서브는 완주했는데도 태그가 나가지 못했다(2026-09-05 캠페인: 기대 3종 중 2종 발행).
#   `cluster` 는 쌍을 **하나의 수행 정체성**으로 본다(멀티는 노드별로 갈라 발행하지 않는다).
#   구분자는 기존 관행대로 `-` 다. 세그먼트를 6개로 늘리지 않는 이유: 모델 슬러그 인덱스를 포함해
#   이름을 해체하는 모든 자리가 5세그먼트를 가정한다 — 축은 arch **안에서** 늘린다.
NODE_AXIS = ("main", "sub", "cluster")
ARCH_SHAPE = re.compile(r"^(?P<hw>[a-z0-9]+)-(?P<node>main|sub|cluster)-(?P<target>[a-z0-9][a-z0-9-]*)$")
# 노드 축이 없던 옛 이름(`gb10-sim-h100`)을 **다른 사유로** 가려내기 위한 형태. 통과시키지 않되,
# "형태 위반" 과 "옛 문법" 을 같은 메시지로 뭉개면 고치는 사람이 무엇을 고쳐야 할지 모른다.
_ARCH_LEGACY_SHAPE = re.compile(r"^[a-z0-9]+-(?!main-|sub-|cluster-)[a-z0-9][a-z0-9-]*$")


def parse_arch_segment(arch: str) -> tuple[str, str, str]:
    """`<hw>-<node>-<target>` → (hw, node, target). **arch 해체의 단일 소유자.**"""
    m = ARCH_SHAPE.match(arch or "")
    if not m:
        die(f"[hint_tag] FAIL: arch 형태 위반(<hw>-<main|sub|cluster>-<target>): {arch!r}")
    return m.group("hw"), m.group("node"), m.group("target")


def arch_violation(arch: str) -> str | None:
    """arch 세그먼트의 위반 사유코드(정상이면 None). 발행기·캠페인 검증기가 **같은 커널**을 쓴다."""
    if not arch:
        return "HINT_ARCH_ABSENT"
    if ARCH_SHAPE.match(arch):
        return None
    if _ARCH_LEGACY_SHAPE.match(arch):
        return "HINT_ARCH_NODE_AXIS_ABSENT"
    return "HINT_ARCH_SHAPE_VIOLATION"


def build_arch_segment(hw: str, node: str, target: str) -> str:
    """축 3개 → arch 세그먼트. 손으로 이어붙이는 자리를 없앤다(문법이 두 벌로 갈라지지 않게)."""
    if node not in NODE_AXIS:
        die(f"[hint_tag] FAIL: 노드 축은 {NODE_AXIS} 중 하나여야 한다: {node!r}")
    arch = f"{hw}-{node}-{target}"
    if (why := arch_violation(arch)) is not None:
        die(f"[hint_tag] FAIL: {why} — 조립한 arch 가 문법을 위반한다: {arch!r}")
    return arch


def parse_hint_tag(name: str) -> tuple[str, str, str, str]:
    """`hint/<vllm>/<model>/<arch>/<recipe>` → 4-튜플. **이름 해체의 단일 소유자.**

    종전에는 `_, vllm, model, arch = tag.split("/")` 가 5곳에 흩어져 있었다. 세그먼트가 하나
    늘면 그 다섯이 **동시에** 깨지고, 하나라도 놓치면 그 경로만 조용히 옛 모양을 가정한다.
    """
    parts = name.split("/")
    if len(parts) != 5 or parts[0] != "hint" or not all(parts):
        die(f"[hint_tag] FAIL: 이름 형태 위반(hint/<vllm>/<model>/<arch>/<recipe>): {name}")
    return parts[1], parts[2], parts[3], parts[4]


def _recipe_token(value: object, prefix: str) -> str | None:
    """인증서 값 하나 → 레시피 토큰. 결측(`N/A`/빈값)은 **토큰을 만들지 않는다**."""
    text = str(value).strip() if value is not None else ""
    if not text or text.upper() == "N/A" or text.lower() in ("none", "null"):
        return None
    slug = re.sub(r"[^a-z0-9]+", "", text.lower())
    return f"{prefix}{slug}" if slug else None


def derive_recipe_segment(fields: dict) -> str:
    """인증서 필드 → 레시피 세그먼트(예 `qmxfp4-len131072-kvfp8`).

    축 3종 = `quantization` · `max_model_len` · `kv_cache_dtype`. 지문 해시가 아니라 **읽히는
    파생값**을 쓰는 이유: 태그가 *지도* 역할을 하려면 이름 자체가 신호를 줘야 하고, 불투명 해시는
    인증서가 이미 드는 값을 두 번째 자리에 적어 `policy:GIT_SINGLE_AUTHORITY` 와 부딪힌다.
    트리플렛 이름은 축이 못 된다 — 실적상 사람이 안 갈라 왔다(같은 모델 인증서 3장이 같은 이름).
    """
    tokens = [t for t in (_recipe_token(fields.get("quantization"), "q"),
                          _recipe_token(fields.get("max_model_len"), "len"),
                          _recipe_token(fields.get("kv_cache_dtype"), "kv")) if t]
    if not tokens:
        die("[hint_tag] FAIL: 인증서에서 레시피 축 3종(quantization·max_model_len·kv_cache_dtype)을 "
            "하나도 읽지 못했다 — 이름을 지어내지 않는다.")
    return "-".join(tokens)


def collide_suffix(name: str, timestamp: str) -> str:
    """파생 세그먼트가 그래도 겹칠 때의 폴백 — `_<timestamp>` 접미사(사용자 결정 2026-09-04).

    ★ **조용히 붙이지 않는다.** 겹쳤다는 것은 *고른 축 3종이 두 레시피를 못 갈랐다*는 신호이고,
    접미사만 붙이고 넘어가면 스키마가 부족하다는 사실이 사라진다(침묵 폴백 금지).
    선례 정합: `doc_naming.bench_filename` 이 같은 시간대 다른 측정에 `_MM_SS` 를 붙이는 것과 동형.
    """
    print(f"[hint_tag] ⚠ 이름 충돌: {name}\n"
          f"        레시피 축 3종(quantization·max_model_len·kv_cache_dtype)이 두 레시피를 "
          f"가르지 못했다. `_{timestamp}` 로 유일화하되 **축이 부족하다는 신호로 기록한다** — "
          f"반복되면 축을 늘려야 한다.", file=sys.stderr)
    return f"{name}_{timestamp}"
HINTS_MARKER = "<!-- hint-index:rows -->"


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


# ── push 자격증명 배선 (2026-09-04 신설) ─────────────────────────────────────
# **왜 필요했나**: 이 스킬의 마지막 단계인 push 에 자격증명 배선이 **아예 없었다**. 호스트에
# credential.helper 가 없으면 `git push` 가 `could not read Username for <원격 호스트>` 로
# 끝나고, 그 사실이 세션마다 재발했다(2026-09-04 실측 — 태그는 sealed 인데 원격 등재만 못 함).
# 운영자는 `envs/.env`(비추적 평면)에 `GITHUB_TOKEN` 을 이미 두고 있었으나 **읽는 코드가 없었다**
# — "만든 것과 도는 것은 다르다" 의 전형이다.
#
# ★ 토큰은 argv·원격 URL·로그 어디에도 넣지 않는다. credential helper 가 **환경변수에서** 읽게
#   해서 프로세스 인자표(`ps`)·reflog·remote.url 에 남지 않게 한다. URL 에 박는 방식
#   (URL 에 토큰을 박는 `https://<token>@<host>/...` 형태)은 절대 쓰지 않는다.
TOKEN_ENV_FILE = ROOT / "envs" / ".env"
PUSH_TOKEN_ENV = "HINT_PUSH_TOKEN"      # helper 가 읽을 임시 변수명(원본 이름과 분리)


def read_push_token(env_file: Path | None = None, environ: dict | None = None) -> str | None:
    """push 용 PAT 을 찾는다: 환경변수 `GITHUB_TOKEN` → `envs/.env` 의 같은 키. 없으면 None.

    비추적 평면에서만 읽는다(`envs/.env` 는 `.gitignore` 대상). 값은 **반환만** 하고 어디에도
    출력하지 않는다.
    """
    environ = os.environ if environ is None else environ
    direct = (environ.get("GITHUB_TOKEN") or "").strip()
    if direct:
        return direct
    f = TOKEN_ENV_FILE if env_file is None else env_file
    if not f.is_file():
        return None
    for line in f.read_text(encoding="utf-8").splitlines():
        st = line.strip()
        if st.startswith("#") or not st.startswith("GITHUB_TOKEN="):
            continue
        val = st.split("=", 1)[1].strip().strip('"').strip("'")
        if val:
            return val
    return None


def push_credential_args() -> list[str]:
    """`git -c` 인자만 산출한다(토큰 없음 — 순수 함수라 자체검사가 argv 를 직접 단언할 수 있다)."""
    helper = ('!f() { test "$1" = get || exit 0; echo username=x-access-token; '
              'echo "password=$%s"; }; f' % PUSH_TOKEN_ENV)
    # 앞의 빈 값이 상속된 helper 목록을 **비운다** — 호스트에 이상한 helper 가 있어도 우리 것만 쓴다.
    return ["-c", "credential.helper=", "-c", "credential.helper=" + helper]


def git_push_authenticated(remote: str, refspec: str, *, dry_run: bool) -> subprocess.CompletedProcess:
    token = read_push_token()
    if not token:
        die("[hint_tag] FAIL: push 자격증명이 없다 — `envs/.env`(비추적)에 `GITHUB_TOKEN=<PAT>` 한 줄을 "
            "두거나 환경변수 `GITHUB_TOKEN` 을 주라. 이 배선이 없으면 무인 실행에서 "
            "`could not read Username for <원격 호스트>` 로 멈춘다.")
    env = dict(os.environ)
    env[PUSH_TOKEN_ENV] = token
    env["GIT_TERMINAL_PROMPT"] = "0"     # 프롬프트 대기 대신 즉시 실패(무인 실행 · 교착 방지)
    args = [*push_credential_args(), "push"]
    if dry_run:
        args.append("--dry-run")
    args += [remote, refspec]
    # ★ dry-run 도 실패를 삼키지 않는다(2026-09-04 감사 C-3). 종전 `check=not dry_run` 은 인증
    #   실패·refspec 거부를 stderr 째 버리고 exit 0 을 냈다 — "실패를 미리 본다" 는 dry-run 의
    #   목적을 정확히 무력화하는 형태다. 원격에 닿지 못하면 dry-run 에서 먼저 죽어야 한다.
    return git(*args, check=True, env=env)


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
#   manifest_ref: <normalized repo-relative manifest path; re-opened and revalidated by every global command>
#   certificate_ref: <manifest-relative certificate path; re-opened and revalidated>
#   -->
#
# All 6 fields are required whenever the block is present at all -- no duplicates, no unrecognized
# keys, no malformed anchor format. Missing the block entirely (a historical/unmigrated tag) and a
# present-but-malformed block are two DISTINCT, stable reason codes.
#
# 2026-09-03 (plan_26090222 F-6a): the three content digests (`manifest_sha256`, `identity_sha256`,
# `certificate_sha256`) were REMOVED -- v1 has no backward-compatible reading of a 9-field block.
# Rationale: the footer's job is to BIND a tag to its evidence *address* (which file, which commit),
# not to re-implement content integrity. `anchor` already pins the exact commit git itself hashes,
# and every referenced path is re-opened + re-validated from ROOT on each read; a hand-carried
# digest beside it is a second authority that can only drift from the first. The digests also
# pointed at `docs/_evidence/*.work-manifest.json`, which is untracked and therefore absent in a
# recipient clone -- so they were unverifiable exactly where the footer travels to.

_FOOTER_MARKER_OPEN = "<!-- hint-evidence-binding:v1"
_FOOTER_MARKER_CLOSE = "-->"
_FOOTER_FIELDS = (
    "version", "tag", "topology", "anchor", "manifest_ref", "certificate_ref",
)
_FOOTER_BLOCK_RE = re.compile(
    re.escape(_FOOTER_MARKER_OPEN) + r"\s*\n(?P<body>.*?)\n" + re.escape(_FOOTER_MARKER_CLOSE),
    re.S,
)
_FOOTER_LINE_RE = re.compile(r"^([a-z0-9_]+): (.*)$")

# ── 폐기된 footer 키의 빈티지 핀 (2026-09-04 신설) ───────────────────────────
# `*_sha256` 3종은 2026-09-03 `policy:GIT_SINGLE_AUTHORITY`(추적물의 digest 를 두 번째 자리에 다시
# 적지 않는다)로 스키마에서 **제거**됐다. 그런데 그 이틀 전에 발행된 태그가 그 키를 들고 있고
# **태그는 불변**이라 고칠 수 없다 → 파서가 거부하면 그 태그 하나가 이후 모든 전수 검증과 push 를
# 영구히 막는다. 이 검증기는 v2 에서 이미 그 상황을 풀어 뒀다고 적어 뒀는데
# (`_require_all_hint_tags_evidence_valid` 독스트링의 "v1 빈티지 → 경고(차단 ✗)"), 그 등급은
# 2026-09-01 에 **"소급 대상 0"** 이라는 이유로 삭제됐다 — 그 전제가 이틀 만에 거짓이 됐다.
#
# ★ 닫힌 목록이다(workflow.md §4종 안티패턴 — 하드코딩의 **정당** 형태 = tripwire). 값은
#   그 태그 오브젝트의 SHA 이며, 태그를 다시 만들면 SHA 가 달라져 자동으로 막힌다.
# ★ 수용은 **읽기 전용 하위호환**이다 — `_build_evidence_footer` 는 `_FOOTER_FIELDS` 만 쓰므로
#   `seal` 이 이 키를 새로 발행하는 경로는 존재하지 않는다(아래 자체검사가 단언한다).
# ★ 값은 **읽고 버린다**. 폐기 이유가 "추적물의 digest 를 두 번째 자리에 적지 마라" 이므로,
#   그 값을 판정에 쓰면 폐기한 의미가 없다.
# ★ 2026-09-04(CP7.5): 이 키들을 **면제받던 태그가 없어졌다.** `LEGACY_FOOTER_TAG_PINS` 는
#   `hint/0.19.1/gpt-oss-120b/gb10-single` 한 건만 담고 있었고 그 태그가 폐기되면서 목록이 비었다 —
#   빈 닫힌 목록은 코드가 아니라 잔재이므로 상수째 걷어낸다. 이제 폐기 키는 **어떤 태그에서도**
#   차단된다(`retired_ok` 기본값 = 빈 집합). 아래 상수는 그 차단이 무엇을 차단하는지 이름 붙이고,
#   자체검사가 "핀 없이는 여전히 막힌다"를 단언하는 데 쓰인다.
RETIRED_FOOTER_KEYS = frozenset({"manifest_sha256", "identity_sha256", "certificate_sha256"})

# ★ 2026-09-04(CP7.5): `LEGACY_CERTIFICATE_REF_ALIASES` 를 걷어냈다. 그 닫힌 목록은
#   `hint/0.19.1/gpt-oss-120b/gb10-single`(태그 오브젝트 27a8a289…) 한 건만 담았고,
#   그 태그가 폐기되면서 어떤 입력에도 매칭될 수 없는 죽은 분기가 됐다. 별칭 없이도
#   정본 판정(“같은 측정인가” = measured 키 동일)은 그대로 산다 — 별칭은 경로만 바꾸는
#   보조 경로였지 판정의 근거가 아니었다.


def _source_anchor_of(tag: str) -> "str | None":
    """태그 페이로드의 `PROVENANCE.json` 이 기록한 **소스 앵커**(산출물을 만든 저장소 커밋). footer 의
    `anchor` 는 페이로드 커밋이라 저장소 트리를 들지 않는다 — 인증서를 git 에서 읽으려면 이쪽이다."""
    hc = git("rev-parse", "--verify", tag + "^{commit}", check=False).stdout.strip()
    if not hc:
        return None
    out = git("show", f"{hc}:PROVENANCE.json", check=False)
    if out.returncode != 0:
        return None
    try:
        doc = json.loads(out.stdout)
    except json.JSONDecodeError:
        return None
    a = doc.get("anchor") if isinstance(doc, dict) else None
    return a if isinstance(a, str) and _FULL_ANCHOR_RE.match(a) else None


def _git_blob_bytes(commit: str, repo_rel: str) -> "bytes | None":
    out = subprocess.run(["git", "cat-file", "-p", f"{commit}:{repo_rel}"], capture_output=True, cwd=str(ROOT))
    return out.stdout if out.returncode == 0 else None


def _certificate_key_from_bytes(data: "bytes | None") -> "tuple | None":
    """바이트 → 측정 키(강한 6키 + measured_utc). 파싱 불가/결측은 None(식별 불가)."""
    if not data:
        return None
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    fields, ok = _cgate().parse_flat_certificate(text)
    return _cgate().certificate_run_key(fields) if ok else None


def _resolve_footer_certificate(tag: str, footer_ref: str, manifest_dir: Path,
                                manifest_cert_ref: "str | None") -> tuple[str, str, "list[str]"]:
    """footer 가 가리키는 인증서를 **워킹트리 → 소스 앵커 트리 → 별칭 핀** 순으로 해소하고 manifest 의
    현재 인증서와 **같은 측정인지** 판정한다(plan_26090410 §3.4).

    반환 (verdict, source, notes):
      verdict ∈ {"same_file", "same_measurement", "absent", "mismatch", "unsafe"}
      source  ∈ {"worktree", "anchor", "alias", "-"}
    `same_measurement` 는 footer 경로 ≠ manifest 경로이지만 두 파일의 측정 키가 같은 경우 — 사본 정리
    뒤 태그(불변)와 manifest(정리됨)가 갈라지는 정상 상태다. 차단하지 않고 notes 로 표면화한다.
    """
    cg = _cgate()
    notes: list[str] = []
    lex_status, parts = cg._lexical_components(manifest_dir, ROOT, footer_ref)
    if lex_status != "ok":
        return "unsafe", "-", notes
    footer_repo_rel = "/".join(parts)

    # ① 워킹트리
    repo_root_fd = os.open(str(ROOT), os.O_RDONLY | os.O_DIRECTORY)
    try:
        r = cg.resolve_and_stat_evidence(repo_root_fd, ROOT, manifest_dir, footer_ref,
                                          expect_dir=False, capture_content=True)
    finally:
        os.close(repo_root_fd)
    footer_bytes, source = None, "-"
    if r["status"] == "ok":
        footer_bytes, source = (r["content_bytes"] or b""), "worktree"
    elif r["status"] != "not_found":
        return "unsafe", "-", notes
    else:
        # ② 소스 앵커 트리 — git 이 드는 바이트가 권위(policy:GIT_SINGLE_AUTHORITY). 워킹트리에서 지운
        #    사본도 그 태그를 만든 커밋에는 남아 있다.
        anchor = _source_anchor_of(tag)
        if anchor:
            blob = _git_blob_bytes(anchor, footer_repo_rel)
            if blob is not None:
                footer_bytes, source = blob, "anchor"
                notes.append(f"HINT_EVIDENCE_CERTIFICATE_REF_RESOLVED_FROM_ANCHOR {footer_repo_rel} @ {anchor[:12]}")

    if manifest_cert_ref is None:
        return ("absent" if footer_bytes is None else "mismatch"), source, notes
    if footer_ref == manifest_cert_ref:
        return ("same_file" if footer_bytes is not None else "absent"), source, notes

    # 경로가 갈린다 → 같은 측정인가?
    repo_root_fd = os.open(str(ROOT), os.O_RDONLY | os.O_DIRECTORY)
    try:
        rm = cg.resolve_and_stat_evidence(repo_root_fd, ROOT, manifest_dir, manifest_cert_ref,
                                           expect_dir=False, capture_content=True)
    finally:
        os.close(repo_root_fd)
    manifest_key = _certificate_key_from_bytes(rm["content_bytes"] if rm["status"] == "ok" else None)
    if footer_bytes is not None:
        footer_key = _certificate_key_from_bytes(footer_bytes)
        if footer_key is not None and footer_key == manifest_key:
            notes.append(f"HINT_EVIDENCE_CERTIFICATE_REF_REBOUND footer={footer_ref!r} → manifest="
                         f"{manifest_cert_ref!r} (same measurement {footer_key[0]}@{footer_key[-1]})")
            return "same_measurement", source, notes
        return "mismatch", source, notes

    # ③ 별칭 핀 경로는 2026-09-04(CP7.5)에 사라졌다 — 유일한 항목의 태그가 폐기됐다.
    #   footer 파일을 어디서도 읽을 수 없으면 그것은 **판정 불가**이며, 판정 불가를 통과로
    #   접지 않는다(부재와 결측의 구분).
    return "absent", source, notes
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


def _parse_evidence_footer(body: str, retired_ok: frozenset = frozenset()) -> dict[str, str]:
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
            if key in retired_ok:
                continue      # 폐기 키 — 빈티지 핀에 한해 읽고 버린다(값은 판정에 쓰지 않는다)
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
        cert_path_str = _binding_artifact_path(manifest)
        if not cert_path_str:
            _die_binding(action, ["HINT_CERTIFICATE_EVIDENCE_MISSING"],
                         {"HINT_CERTIFICATE_EVIDENCE_MISSING":
                          "manifest has no evidence.certificate.path -- cannot bind a durable "
                          "evidence footer without a certificate artifact to hash"},
                         identity, manifest.get("task_class"))
        # capture_content=True: 레시피 세그먼트를 이 바이트로 대조한다(CP7). 경로를 다시 열지 않고
        # **이미 안전 해소된 그 바이트**를 쓰는 것이 요점이다 — 두 번 열면 TOCTOU 창이 생긴다.
        r_cert = _cgate().resolve_and_stat_evidence(repo_root_fd, ROOT, resolved_manifest_path.parent,
                                                     cert_path_str, expect_dir=False,
                                                     capture_content=True)
        if r_cert["status"] != "ok":
            _die_binding(action, ["HINT_CERTIFICATE_UNSAFE_OR_MISSING"],
                         {"HINT_CERTIFICATE_UNSAFE_OR_MISSING":
                          f"certificate {cert_path_str!r} could not be safely resolved+hashed "
                          f"(status={r_cert['status']}) -- refusing despite the common gate's own approval"},
                         identity, manifest.get("task_class"))
    finally:
        os.close(repo_root_fd)

    # ★ 레시피 세그먼트는 **손저작이 아니라 파생값**이다(CP7 · plan_26090415 §7.5 M1). 여기서
    #   바인딩된 인증서로부터 다시 파생해 대조한다 — 이름이 측정과 어긋나면 그 태그는 지도가
    #   아니라 오도(誤導)다. `derive_slug` 가 모델 슬러그에 대해 하는 일과 같은 처방(작명 자유도
    #   제거)이며, 인증서를 못 읽으면 대조를 **건너뛰지 않고** 거부한다.
    _tag_recipe = parse_hint_tag(tag)[3]
    _ev = manifest.get("evidence") if isinstance(manifest, dict) else None
    _ev = _ev if isinstance(_ev, dict) else {}
    _cert_decl = _ev.get("certificate")
    _bound_is_certificate = isinstance(_cert_decl, dict) and bool(_cert_decl.get("path"))
    if _bound_is_certificate:
        _cert_bytes = r_cert.get("content_bytes")
        if _cert_bytes:
            _cert_fields, _cert_ok = _cgate().parse_flat_certificate(
                _cert_bytes.decode("utf-8", "replace"))
        else:
            _cert_fields, _cert_ok = {}, False
        if not _cert_ok:
            _die_binding(action, ["HINT_CERTIFICATE_UNPARSEABLE"],
                         {"HINT_CERTIFICATE_UNPARSEABLE":
                          f"certificate {cert_path_str!r} did not parse as a flat certificate -- "
                          f"refusing to bind a recipe segment that cannot be checked against it"},
                         identity, manifest.get("task_class"))
        _expected_recipe = derive_recipe_segment(_cert_fields)
        if _tag_recipe.split("_")[0] != _expected_recipe:   # `_<timestamp>` 충돌 폴백을 벗긴다
            _die_binding(action, ["HINT_RECIPE_SEGMENT_MISMATCH"],
                         {"HINT_RECIPE_SEGMENT_MISMATCH":
                          f"tag recipe segment {_tag_recipe!r} does not match the segment derived "
                          f"from the bound certificate ({_expected_recipe!r}) -- the name must be "
                          f"derived, not authored; run "
                          f"`hint_tag.py recipe-segment --certificate {cert_path_str}`"},
                         identity, manifest.get("task_class"))
    else:
        # 인증서가 **애초에 존재할 수 없는** 발행 경로다(perf_waiver · explore — 인증서는 PASS
        # 때만 나온다). 그때 바인딩 대상은 항상 발행되는 bench_report 이고, 리포트는 flat
        # 인증서가 아니므로 레시피 세그먼트를 측정과 대조할 근거가 **없다**.
        # 조용히 넘기지 않는다 — 무엇을 확인하지 못했는지 큰 소리로 남긴다(대조 부재 ≠ 대조 통과).
        print(f"[hint_tag] ⚠ 레시피 세그먼트 대조 생략: 바인딩 대상이 인증서가 아니라 "
              f"{cert_path_str!r} 다(인증서는 PASS 때만 발행된다). 세그먼트 {_tag_recipe!r} 의 "
              f"형태는 검증됐으나 **측정과의 일치는 검증되지 않았다**.", file=sys.stderr)

    # r_manifest / r_cert 의 status 검사는 위에서 이미 끝났다 — footer 는 그 **주소**만 싣고
    # digest 는 싣지 않는다(F-6a). 안전 resolve 자체가 발행 시점 게이트이고, 읽는 쪽은 매 판독마다
    # ROOT 기준으로 같은 resolve 를 다시 돌린다.
    return {
        "version": "1", "tag": tag, "topology": topology, "anchor": anchor,
        "manifest_ref": manifest_ref, "certificate_ref": cert_path_str,
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


def _existing_model_slugs() -> dict[str, str]:
    """발행된 hint 태그에서 파생한 {정규화키: 먼저 발행된 철자}.

    ★ 종전엔 `CANONICAL_SLUGS` 라는 손저작 표가 이 일을 했다. 2026-08-20 실측에서 **발행된 32
    슬러그 중 27종이 그 표 밖**이었다(`--allow-new-slug` 로 통과) — 표는 이미 집행되지 않고
    있었고, 모델이 늘 때마다 스크립트를 고쳐야 했다. **파생 가능한데 손으로 적은 것**이므로 없앤다.

    남는 일은 의미 판정이 아니라 **사실 대조**다: "이 철자가 이미 발행된 것과 대소문자·구두점만
    다른가". 무엇이 같은 모델인가(패밀리)는 이 스크립트가 판단하지 않는다 — Agent 가 인용과 함께
    판단하고 `hints/families.json` 이 담는다(결정론↔Agent 경계).
    """
    out: dict[str, str] = {}
    for t in existing_hint_tags():
        parts = t.split("/")
        if len(parts) == 5:
            out.setdefault(_norm_slug(parts[2]), parts[2])
    return out


def existing_hint_tags() -> list[str]:
    return [t for t in git("tag", "-l", "hint/*").stdout.split() if t]


def validate_name(name: str, expect_absent: bool = True) -> tuple[str, str, str, str]:
    if not TAG_SHAPE.match(name):
        die(f"[hint_tag] FAIL: 이름 형태 위반(hint/<vllm>/<model>/<arch>/<recipe>): {name}")
    if git("check-ref-format", f"refs/tags/{name}", check=False).returncode != 0:
        die(f"[hint_tag] FAIL: git 이 거부하는 ref 이름: {name}")
    vllm, model, arch, recipe = parse_hint_tag(name)
    if not RECIPE_SHAPE.match(recipe):
        die(f"[hint_tag] FAIL: 레시피 세그먼트 형태 위반(소문자 슬러그): {recipe!r}\n"
            f"        레시피는 **인증서에서 파생**한다 — `hint_tag.py recipe-segment --certificate <경로>` "
            f"가 그 값을 낸다. 손으로 짓지 않는다.")
    # arch 세그먼트는 **어느 노드 형상이 수행했는가**를 담는다(2026-09-06 · plan_26090616 Q1/Q2).
    # 이름이 곧 증거 연결이므로 여기서 문법을 집행한다 — 옛 이름과 형태 위반을 **다른 사유로** 가른다.
    if (why := arch_violation(arch)) is not None:
        if why == "HINT_ARCH_NODE_AXIS_ABSENT":
            die(f"[hint_tag] FAIL: {why} — arch 세그먼트 {arch!r} 에 **노드 축이 없다**.\n"
                f"        문법: <hw>-<main|sub|cluster>-<target>  (예 gb10-main-sim-h100 · "
                f"gb10-sub-sim-h100 · gb10x2-cluster-sim-h100)\n"
                f"        hint 태그는 '이 조합이 된다' 가 아니라 **'어느 노드 형상이 수행한 기록'** 이다. "
                f"축이 없으면 같은 하드웨어의 메인·서브가 같은 이름을 원하고, 그때 서브의 기록은 "
                f"완주했어도 발행되지 못한다(2026-09-05 실측).")
        die(f"[hint_tag] FAIL: {why} — arch 세그먼트 형태 위반: {arch!r} "
            f"(소문자 <hw>-<main|sub|cluster>-<target>)")
    prior = _existing_model_slugs().get(_norm_slug(model))
    if prior is not None and prior != model:
        die(f"[hint_tag] FAIL: 모델 슬러그 '{model}' 은 이미 발행된 '{prior}' 와 대소문자·구두점만 "
            f"다르다(정규화키 동일). 정본 철자 '{prior}' 를 쓰라 — 철자 분열은 "
            f"`git tag -l 'hint/*/<slug>/*/*'` 검색을 조용히 깨뜨린다(2026-08-20 실측 2건).")
    existing = existing_hint_tags()
    if expect_absent and name in existing:
        die(f"[hint_tag] FAIL: 이미 존재하는 태그: {name}")
    for e in existing:
        if e == name:
            continue
        if e.startswith(name + "/") or name.startswith(e + "/"):
            die(f"[hint_tag] FAIL: 기존 태그와 D/F prefix 충돌: {e}")
    return vllm, model, arch, recipe


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
    vllm, model, arch, recipe = validate_name(a.tag)
    slug_src = require_derived_slug(model, vllm, arch, a.hf_repo, a.model_path)
    print(f"[hint_tag] 슬러그 '{model}' 확인 (출처: {slug_src})")
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    manifest, _ = _load_manifest_for_binding("hint_create", a.manifest)
    _require_hint_promotion_target("hint_create", manifest, tag=a.tag, topology=a.topology,
                                    anchor=anchor, vllm=vllm, model=model)
    _require_serving_evidence("hint_create", manifest, getattr(a, "payload", None))

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
    vllm, model, arch, recipe = validate_name(a.tag)  # 미존재·철자충돌·합법 재확인
    require_derived_slug(model, vllm, arch, a.hf_repo, a.model_path)
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    manifest, resolved_manifest_path = _load_manifest_for_binding("hint_finalize", a.manifest)
    _require_hint_promotion_target("hint_finalize", manifest, tag=a.tag, topology=a.topology,
                                    anchor=anchor, vllm=vllm, model=model)
    # ── 감사 심각도1-① 폐쇄 (2026-09-01 · plan §12 A4-a)
    # 계약 §3 의 발행조건 A·B 를 검사하는 이 함수가 **`create` 에만** 걸려 있었다. `create` 의
    # 부작용은 스캐폴드 파일 하나뿐이라, `--recipe <임의경로>` 로 `seal` 에 직행하면 비용 0으로
    # 우회됐다 — 계약 §2 가 지목한 **유일한 위협**("서빙 실패를 성공으로 허위기재한 정보의 배포")
    # 에 대한 주 방어선이 실제 발행 경로에 없었던 것이다.
    # 여기는 `finalize`·`seal` 이 **공유하는** 지점이고 태그 생성보다 앞이므로, 한 줄로 양쪽이
    # 닫히고 거부 시 부작용이 0이다(승격게이트가 지킨 규율과 같다).
    _require_serving_evidence("hint_finalize", manifest, getattr(a, "payload", None))
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

    # 기본값은 **합성 프로젝트 신원**이다 — git config 가 아니다(2026-09-01 교정).
    # 이전 기본값은 `git config user.name/email` 이었고, 그 결과 발행자의 실명·업무 이메일이
    # 배포 태그 오브젝트에 박혔다(실제 발생 — push 전에 회수). tagger PII 검사는 generic
    # `email` 패턴을 **건너뛰므로**(tagger 는 정의상 이메일을 갖는다) 리터럴 목록에 없는 실제
    # 주소는 그대로 통과한다. 즉 "잊으면 새는" 구조였다.
    # 안전한 쪽을 기본값으로 두고, 실명을 쓰려면 **명시**하게 한다.
    tname = a.tagger_name or DEFAULT_TAGGER_NAME
    temail = a.tagger_email or DEFAULT_TAGGER_EMAIL
    idhits = scan_text(f"{tname} {temail}", terms, skip_generic=frozenset({"email"}))
    if idhits:
        die("[hint_tag] FAIL(tagger PII): tagger 신원이 PII 를 흘림: " + ", ".join(idhits)
            + "\n  → --tagger-name/--tagger-email 로 clean 한 공개 신원을 지정하세요"
            " (배포 태그 오브젝트에 tagger 가 박힙니다).")

    # The tag message = original recipe body + footer, streamed via stdin (`-F -`) -- the source
    # recipe FILE on disk is never rewritten (design requirement: finalize must not mutate it).
    # --cleanup=verbatim is REQUIRED: git's default cleanup ('strip') silently drops every line
    # starting with '#' -- which is every '## N. <heading>' section marker this template mandates
    # (lint_body's L1 checks). Without this flag, `git tag -a -F -` strips all section headings
    # from the resulting tag object even though lint_body validated them present in `body` --
    # discovered 2026-08-20 when hint_tag push's own re-validation (against the actual tag body,
    # not the source recipe file) rejected a freshly-finalized tag for missing '## 1. 벽 지도' that
    # was plainly present in the recipe on disk. Root-caused via `git --version` sandbox repro
    # (git 2.43.0: `git tag -a -F -` with '## heading' input drops the heading line). This is the
    # same defect the template's own header comment misattributed to authors not following the
    # template ("발행된 49/49 태그에 헤딩이 0개") -- it was this git default the whole time.
    env = dict(os.environ, GIT_COMMITTER_NAME=tname, GIT_COMMITTER_EMAIL=temail)
    tag_message = (body if body.endswith("\n") else body + "\n") + "\n" + footer_text
    git("-c", f"user.name={tname}", "-c", f"user.email={temail}",
        "tag", "-a", a.tag, anchor, "-F", "-", "--cleanup=verbatim", env=env, input_text=tag_message)
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
    _wrote_md = _hints_regen(idx["hints"])
    print(f"[hint_tag] index.json{' + HINTS.md' if _wrote_md else ''} 카탈로그 인덱스 갱신 완료."
          + ("" if _wrote_md else "  (HINTS.md 는 갱신되지 않았다 — 위 경고 참조)"))
    return 0


def _hints_row(e: dict) -> str:
    return (f"| `{e['tag']}` | {e['vllm']} | {e['model']} | {e['arch']} | "
            f"{e.get('recipe','')} | "
            f"{e.get('topology','')} | {e.get('status','active')} | "
            f"{e.get('superseded_by') or e.get('related') or '—'} | "
            f"{e.get('last_verified','')} | {e.get('brief','')} |")


def _hints_regen(hints: list[dict]) -> bool:
    """HINTS.md 카탈로그 행 전량 재생성(index = 진실원천). 마커 앞에 정렬 삽입.

    → 실제로 갱신했으면 True. **호출부는 이 값을 보고 보고 문구를 정해야 한다** — 종전엔 파일이
    없거나 마커가 없어도 조용히 반환하면서 `cmd_index` 가 "HINTS.md 갱신 완료"를 무조건 찍었다.
    하지 않은 일을 했다고 말하는 침묵 no-op 이다(2026-09-01 E2E 에서 검출).
    """
    if not HINTS_FILE.is_file():
        print(f"[hint_tag] ⚠ {HINTS_FILE.name} 이 없어 카탈로그 행을 쓰지 못했다 — index.json 만 갱신됐다.",
              file=sys.stderr)
        return False
    if HINTS_MARKER not in HINTS_FILE.read_text(encoding="utf-8"):
        print(f"[hint_tag] ⚠ {HINTS_FILE.name} 에 삽입 마커({HINTS_MARKER}) 가 없어 행을 쓰지 못했다 "
              f"— index.json 만 갱신됐다.", file=sys.stderr)
        return False
    out = []
    for ln in HINTS_FILE.read_text(encoding="utf-8").splitlines():
        if ln.startswith("| `hint/"):
            continue  # 기존 hint 행 전부 제거
        if ln.strip() == HINTS_MARKER:
            out.extend(_hints_row(e) for e in hints)
        out.append(ln)
    HINTS_FILE.write_text("\n".join(out) + "\n", encoding="utf-8")
    return True


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


def _validate_hint_tag_evidence(tag: str, action: str) -> tuple[dict[str, str] | None, list[tuple[str, str]]]:
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
        return None, [("HINT_TAG_NOT_ANNOTATED", f"{tag}: HINT_TAG_NOT_ANNOTATED annotated 태그가 아님(type={typ})")]
    # 폐기 footer 키를 면제받는 태그는 더 이상 없다(CP7.5) — 기본값 = 빈 집합 = 전면 차단.
    try:
        footer = _parse_evidence_footer(_tag_object_body(tag))
    except HintEvidenceBindingError as e:
        return None, [(e.code, f"{tag}: {e.code} {e.message}")]
    # ★ 문제는 (code, message) 로 낸다 — 판정은 code 로만 한다. 종전엔 렌더된 message 를
    # 부분일치(`CODE in msg`)로 분류했는데, message 에는 footer 값이 그대로 박히므로
    # 발행자가 footer 필드에 코드 문자열을 넣어 등급을 뒤집을 수 있었다(감사 심각도1-②, 주입 실증).
    problems: list[tuple[str, str]] = []

    if footer["tag"] != tag:
        problems.append(("HINT_EVIDENCE_BINDING_TAG_SELF_MISMATCH", f"{tag}: HINT_EVIDENCE_BINDING_TAG_SELF_MISMATCH footer.tag={footer['tag']!r} != actual tag {tag!r}"))

    actual_commit = git("rev-parse", "--verify", tag + "^{commit}", check=False).stdout.strip()
    if not actual_commit:
        problems.append(("HINT_TAG_COMMIT_UNRESOLVABLE", f"{tag}: HINT_TAG_COMMIT_UNRESOLVABLE 태그의 peeled commit 을 확인할 수 없음"))
    elif footer["anchor"] != actual_commit:
        problems.append(("HINT_EVIDENCE_BINDING_ANCHOR_SELF_MISMATCH", f"{tag}: HINT_EVIDENCE_BINDING_ANCHOR_SELF_MISMATCH footer.anchor={footer['anchor']!r} "
                         f"!= actual peeled commit {actual_commit!r}"))

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
        problems.append((_code, f"{tag}: {_code} manifest_ref "
                         f"{footer['manifest_ref']!r} resolve 실패(status={r_manifest['status']})"))
        return footer, problems  # nothing further can be checked without the manifest bytes
    try:
        ref_manifest = json.loads((r_manifest["content_bytes"] or b"").decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        problems.append(("HINT_EVIDENCE_MANIFEST_INVALID_JSON", f"{tag}: HINT_EVIDENCE_MANIFEST_INVALID_JSON manifest_ref {footer['manifest_ref']!r} "
                         f"가 유효한 JSON 이 아님"))
        return footer, problems
    if not isinstance(ref_manifest, dict):
        problems.append(("HINT_EVIDENCE_MANIFEST_INVALID_SHAPE", f"{tag}: HINT_EVIDENCE_MANIFEST_INVALID_SHAPE manifest_ref root 가 JSON object 아님"))
        return footer, problems

    manifest_ref_abs = ROOT / footer["manifest_ref"]  # already proven safe/regular above

    # ---- 인증서 참조 해소를 **게이트 앞에** 둔다(감사 C-1: 게이트가 먼저 NOT_PROMOTION_READY 를 붙이면
    #      `_ABSENT_CODES` 의 인증서 항목이 도달 불가가 되어 부재가 "forged/drifted" 로 오진됐다).
    ref_cert_path = _binding_artifact_path(ref_manifest)
    cert_verdict, cert_source, cert_notes = _resolve_footer_certificate(
        tag, footer["certificate_ref"], manifest_ref_abs.parent, ref_cert_path)
    for note in cert_notes:
        print(f"[hint_tag] NOTE {tag}: {note}", file=sys.stderr)
    if cert_verdict == "unsafe":
        problems.append(("HINT_EVIDENCE_CERTIFICATE_UNSAFE_OR_MISSING",
                         f"{tag}: HINT_EVIDENCE_CERTIFICATE_UNSAFE_OR_MISSING certificate_ref "
                         f"{footer['certificate_ref']!r} resolve 실패(escape/symlink/type)"))
    elif cert_verdict == "absent":
        problems.append(("HINT_EVIDENCE_CERTIFICATE_REF_ABSENT",
                         f"{tag}: HINT_EVIDENCE_CERTIFICATE_REF_ABSENT certificate_ref "
                         f"{footer['certificate_ref']!r} 를 워킹트리·소스 앵커 트리·별칭 핀 어디에서도 읽을 수 없다"))
    elif cert_verdict == "mismatch":
        problems.append(("HINT_EVIDENCE_CERTIFICATE_REF_MISMATCH",
                         f"{tag}: HINT_EVIDENCE_CERTIFICATE_REF_MISMATCH footer.certificate_ref="
                         f"{footer['certificate_ref']!r} 와 manifest evidence.certificate.path={ref_cert_path!r} 가 "
                         f"**다른 측정**을 증명한다(같은 측정이면 REBOUND 로 통과한다)"))

    gate_ok, gate_detail = _evaluate_manifest_promotion_ready(manifest_ref_abs, action)
    if not gate_ok:
        problems.append(("HINT_EVIDENCE_MANIFEST_NOT_PROMOTION_READY", f"{tag}: HINT_EVIDENCE_MANIFEST_NOT_PROMOTION_READY {gate_detail}"))

    vllm, model, arch, recipe = parse_hint_tag(tag)
    ref_pt = ref_manifest.get("promotion_target")
    if not isinstance(ref_pt, dict):
        problems.append(("HINT_EVIDENCE_PROMOTION_TARGET_MISSING", f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_MISSING referenced manifest 에 promotion_target 없음"))
    else:
        if ref_pt.get("kind") != "hint":
            problems.append(("HINT_EVIDENCE_PROMOTION_TARGET_KIND_MISMATCH", f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_KIND_MISMATCH kind={ref_pt.get('kind')!r}"))
        if ref_pt.get("tag") != tag:
            problems.append(("HINT_EVIDENCE_PROMOTION_TARGET_TAG_MISMATCH", f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_TAG_MISMATCH "
                             f"promotion_target.tag={ref_pt.get('tag')!r} != {tag!r}"))
        if ref_pt.get("topology") != footer["topology"]:
            problems.append(("HINT_EVIDENCE_PROMOTION_TARGET_TOPOLOGY_MISMATCH", f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_TOPOLOGY_MISMATCH "
                             f"promotion_target.topology={ref_pt.get('topology')!r} != footer topology {footer['topology']!r}"))
        if ref_pt.get("anchor") != footer["anchor"]:
            problems.append(("HINT_EVIDENCE_PROMOTION_TARGET_ANCHOR_MISMATCH", f"{tag}: HINT_EVIDENCE_PROMOTION_TARGET_ANCHOR_MISMATCH "
                             f"promotion_target.anchor={ref_pt.get('anchor')!r} != footer anchor {footer['anchor']!r}"))

    ref_identity = ref_manifest.get("identity") or {}
    if ref_identity.get("model") != model:
        problems.append(("HINT_EVIDENCE_IDENTITY_MODEL_MISMATCH", f"{tag}: HINT_EVIDENCE_IDENTITY_MODEL_MISMATCH identity.model={ref_identity.get('model')!r} != tag model {model!r}"))
    m_vllm = ref_identity.get("vllm")
    if not (m_vllm == vllm or (isinstance(m_vllm, str) and m_vllm.startswith(vllm + "-"))):
        problems.append(("HINT_EVIDENCE_IDENTITY_VLLM_MISMATCH", f"{tag}: HINT_EVIDENCE_IDENTITY_VLLM_MISMATCH identity.vllm={m_vllm!r} does not bind to tag vllm {vllm!r}"))
    expected_class = _topology_class(footer["topology"])
    if ref_identity.get("topology") != expected_class:
        problems.append(("HINT_EVIDENCE_IDENTITY_TOPOLOGY_CLASS_MISMATCH", f"{tag}: HINT_EVIDENCE_IDENTITY_TOPOLOGY_CLASS_MISMATCH identity.topology="
                         f"{ref_identity.get('topology')!r} != topology class {expected_class!r}"))
    tp_label = _TP_LABEL_RE.search(footer["topology"])
    if tp_label is not None and ref_identity.get("tp") != int(tp_label.group(1)):
        problems.append(("HINT_EVIDENCE_IDENTITY_TP_MISMATCH", f"{tag}: HINT_EVIDENCE_IDENTITY_TP_MISMATCH identity.tp={ref_identity.get('tp')!r} "
                         f"!= TP{tp_label.group(1)} in topology label {footer['topology']!r}"))

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
    _ev_blocking, _ev_unver = classify_evidence_problems(
        [(t, _validate_hint_tag_evidence(t, "hint_verify")[1]) for t in tags])
    problems.extend(_ev_blocking)
    # 발행자 평면(verify=릴리즈 게이트)에서는 **부재도 차단**이다 — 발행자는 자기가 주장하는 증거를
    # 갖고 있어야 한다. 수신자 평면(collect·reindex)에서만 부재를 경고로 낮춘다(사용자 결정 α).
    problems.extend(f"{x}: HINT_EVIDENCE_REF_ABSENT 참조 증거가 이 체크아웃에 없다 — "
                    "발행자는 증거를 보유해야 한다" for x in _ev_unver)
    problems.extend(f"{x}: HINT_TAG_UNSEALED {_r}" for x in tags
                    if (_r := unsealed_reason(x)))

    for bp in sorted(ROOT.glob("output/*/build_patches/*.sh")):
        h = scan_text(bp.read_text(encoding="utf-8", errors="ignore"), terms)
        if h:
            problems.append(f"{bp.relative_to(ROOT)}: build_patch PII {h}")

    if terms is None:
        problems.append("pii_terms.txt 부재 → generic 패턴만으로 스캔(축소 커버리지)")

    if problems:
        print("[hint_tag] VERIFY FAIL:")
        for p in problems:
            print("  -", p)
        return 1
    print(f"[hint_tag] VERIFY PASS: hint 태그 {len(tags)}개 · index 정합 · 태그오브젝트/build_patches PII-clean.")
    return 0


def _no_cert_binding_source(manifest: dict) -> "str | None":
    """인증서가 없을 때 **무엇이 bench_report 바인딩을 여는가**의 단일 판정자.

    인증서는 PASS 때만 발행된다(`publish_benchmark_record` — 그 규칙은 유지한다). 그래서 "인증서
    부재"는 하나의 사실이 아니라 서로 다른 두 사실일 수 있다:

      · `perf_waiver` — 성능 REFUTE 를 **사람이 서명**해 통과시킨 경로(positive key).
      · `explore`     — 사용자 HITL 탐색 트리거로 루브릭 권한이 explore 인 런. 성능 판정이 게이트가
                        아니라 **서술**이므로 REFUTE 여도 승격이 열린다(completion_gate U1).
                        explore 는 waiver 를 요구하지 **않는다** — 요구하면 그 자동개방이 여기서
                        다시 죽는다(2026-08-24 실측한 그 죽은 코드의 hint 평면 쌍둥이).

    둘 다 아니면 None(=차단). 요구를 낮추는 것이 아니라 **바인딩 대상 문서가 다를 뿐**이며, lite
    실측 자체는 어느 경로든 **항상 발행되는 bench_report** 에 실려 있다.

    ★ 이 판정이 여러 곳에 각각 박히면 즉시 불일치가 난다(`_binding_artifact_path` 주석의 2026-08-01
      실측: finalize 는 리포트로 묶었는데 verify 는 인증서와 대조해 FAIL). 그래서 발행 조건 검사
      (`_require_serving_evidence`)와 바인딩 대상 선택(`_binding_artifact_path` → finalize footer ·
      verify 대조)이 **모두 이 한 함수만** 본다.

    explore 계약은 `completion_gate._manifest_rubric_contract` 를 **그대로 재사용한다**(포인터 원칙).
    floor>0(공허 PASS 배제) ∧ ratio 유한 ∧ primary_source 실재 ∧ authority 값역 ∧ rubric_source
    출처표시 — 이걸 여기서 손으로 다시 적으면 승격 게이트와 발행 게이트의 판정이 갈라진다.
    """
    if not isinstance(manifest, dict):
        return None
    benchmark = manifest.get("benchmark")
    benchmark = benchmark if isinstance(benchmark, dict) else {}
    if benchmark.get("perf_waiver"):
        return "perf_waiver"
    contract = getattr(_cgate(), "_manifest_rubric_contract", None)
    if contract is None:
        # fail-loud: 침묵 폴백(=조용히 차단)이면 "explore 인데 왜 막혔나"를 영원히 못 읽는다.
        die("[hint_tag] FAIL(fail-closed): completion_gate.py 에 _manifest_rubric_contract 가 없다 "
            "— explore 루브릭 권한 계약을 검증할 수 없어 인증서-부재 바인딩을 열지 않는다 "
            "(동거 사본이 낡았을 수 있다: sync_to_sub 로 재배달하라).")
    rubric = contract(benchmark)
    if rubric.get("declared") and rubric.get("ok") and rubric.get("authority") == "explore":
        return "explore"
    return None


def _payload_declared_missing(payload_dir) -> frozenset:
    """조립 중인 페이로드 디렉터리가 선언한 결손 코드. 경로가 없거나 못 읽으면 **빈 집합**이다 —
    읽지 못한 것을 "선언됐다" 로 처리하면 게이트가 스스로 열린다."""
    if not payload_dir:
        return frozenset()
    path = Path(payload_dir) / "PAYLOAD.json"
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return frozenset()
    missing = doc.get("missing") if isinstance(doc, dict) else None
    return frozenset(m for m in (missing or []) if isinstance(m, str))


def _require_serving_evidence(action: str, manifest: dict, payload_dir=None) -> None:
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
    # ★ v4 절단선(2026-09-06 · plan_26090616 Q7/Q8): 두 조건의 **성질이 다르다**.
    #   A(서빙 성공)는 계약 §2 가 지목한 유일한 위협의 방어선이라 그대로 **차단**이다 — 실패를
    #     성공으로 위장한 배포를 막는 자리이고, 서빙 실패 셀은 애초에 독립 태그를 갖지 않는다
    #     (형제 태그의 벽 지도로 실린다 · 계획 §범위 밖).
    #   B(lite 정량지표)는 **선택**으로 내려간다. 인증서·리포트 발행은 벤치마커의 책임이고,
    #     캠페인 종료와 hint 발행은 독립 사건이다. 부재를 차단으로 두면 발행돼야 할 hint 가 안
    #     나가고, 수신자에게 "발행 안 됨" 은 "시도된 적 없음" 과 구분되지 않는다.
    #   단 **적히지 않은 부재는 여전히 차단**이다 — 면제되는 것은 "없다" 가 아니라 "없다고 적혀 있다".
    problems: list[str] = []          # A — 무조건 차단
    quant_missing: list[str] = []     # B — 선언돼 있으면 통과, 아니면 차단
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
    #    ★ 단 **인증서가 구조적으로 존재할 수 없는 두 경로**(perf_waiver · explore)가 있다 —
    #      publish_benchmark_record 가 PASS 때만 인증서를 내기 때문이다(그 규칙은 유지한다).
    #      그러나 lite 실측 자체는 **항상 발행되는 bench_report** 에 실려 있으므로, 그 두 경로에서는
    #      리포트를 B 의 근거로 삼는다. "증거가 없다"가 아니라 "증거가 다른 문서에 있다" 이므로
    #      요구 강도를 낮추는 것이 아니다. 어느 경로가 열렸는지의 판정은
    #      `_no_cert_binding_source` 단독 소유다(바인딩 대상과 발행 조건이 갈리지 않도록).
    ev = manifest.get("evidence") if isinstance(manifest.get("evidence"), dict) else {}
    cert_rel = (ev.get("certificate") or {}).get("path")
    no_cert_source = _no_cert_binding_source(manifest) if not cert_rel else None
    if not cert_rel and no_cert_source:
        rep_rel = (ev.get("bench_report") or {}).get("path")
        if not rep_rel:
            quant_missing.append("HINT_MISSING_BENCH_REPORT")
        else:
            rep_path = (ROOT / "docs" / "_evidence" / rep_rel).resolve()
            try:
                rtext = rep_path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                rtext = ""
                quant_missing.append("HINT_MISSING_BENCH_REPORT")
            if rtext:
                if "lite 지표" not in rtext:
                    quant_missing.append("HINT_MISSING_LITE")
                if not re.search(r"gen tokens/sec[^|]*\|\s*[0-9]", rtext):
                    quant_missing.append("HINT_MISSING_LITE")
    elif not cert_rel:
        quant_missing.append("HINT_MISSING_CERTIFICATE")
    else:
        cert_path = (ROOT / "docs" / "_evidence" / cert_rel).resolve()
        try:
            text = cert_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            text = ""
            quant_missing.append("HINT_MISSING_CERTIFICATE")
        if text:
            if not re.search(r"(?m)^lite_included:\s*true\b", text):
                quant_missing.append("HINT_MISSING_LITE")
            if not re.search(r"(?m)^lite_gen_tps_warm:\s*[0-9]", text):
                quant_missing.append("HINT_MISSING_LITE")
    # B 의 결손은 **페이로드가 선언했는가**로 갈린다.
    if quant_missing:
        declared = _payload_declared_missing(payload_dir)
        undeclared = sorted(set(quant_missing) - declared)
        if undeclared:
            problems.append(
                "정량지표 결손이 페이로드 `missing[]` 에 선언되지 않았다: " + ", ".join(undeclared)
                + " — 적히지 않은 부재는 침묵이다(계약 v4 §5). `hint_collect` 가 이 코드를 적게 하라.")
        else:
            print(f"[hint_tag] NOTE {action}: 선언된 정량지표 결손이라 차단하지 않는다 — "
                  f"{', '.join(sorted(set(quant_missing)))} (강행 발행 · 계약 v4 §3)", file=sys.stderr)
    if problems:
        _die_binding(action, ["HINT_SERVING_EVIDENCE_INSUFFICIENT"],
                     {"HINT_SERVING_EVIDENCE_INSUFFICIENT":
                      "hints/HINT_ISSUANCE_CONTRACT.md §3 발행 조건 미충족 — 서빙 성공(A)은 무조건, "
                      "정량지표(B)는 **선언되지 않은 부재**일 때 차단된다:\n  " + "\n  ".join(problems)},
                     manifest.get("identity") if isinstance(manifest, dict) else None,
                     manifest.get("task_class") if isinstance(manifest, dict) else None)


def _binding_artifact_path(manifest: dict) -> "str | None":
    """footer 가 해시로 묶을 **계측 산출물 경로**의 단일 소유자.

    통상은 인증서다. 단 인증서가 애초에 존재할 수 없는 경로가 둘 있고(perf_waiver · explore —
    publish_benchmark_record 가 PASS 때만 발행하며 그 규칙은 유지) 그때는 **항상 발행되는
    bench_report** 를 바인딩 대상으로 삼는다. 요구를 낮추는 게 아니라 대상 문서가 다를 뿐이다.
    어느 경로가 열렸는지는 `_no_cert_binding_source` 가 단독으로 판정한다.

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
    if _no_cert_binding_source(manifest):
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


def unsealed_reason(tag: str) -> str | None:
    """이 태그가 `seal` 산출물이 **아님**을 태그 오브젝트만으로 판정한다.

    ★ 외부 파일이 필요 없다 — 그래서 **증거가 배포되지 않는 수신자 평면에서도 그대로 작동하는
    유일한 검출기**다. digest 계열 검사는 증거 파일이 있어야 하므로 수신자에게는 항상 '부재'로
    떨어진다(plan_26082017 §10.4). 손으로 만든 footer 는 digest 로는 못 잡아도 여기서 잡힌다:
    `cmd_finalize` 가 lint_body 를 die 로 집행하므로, **린트를 못 넘는 본문은 seal 이 낸 것이
    아니다.** 2026-08-20 실증(태그 4건).

    ★ 2026-09-01 개정: 종전엔 태그의 taggerdate 가 린터 도입 시각보다 이르면 검사를 **건너뛰었다**.
    그 시각은 태그 오브젝트 안에 있어 **발행자가 정한다** — 피감사자가 자기 검사를 끌 수 있는
    구조였다(감사 심각도1-③). 과거 태그 전량 초기화로 소급 면제의 근거도 사라져 게이트를 제거한다.
    이제 모든 hint 태그가 예외 없이 이 검사를 받는다."""
    if git("cat-file", "-t", tag, check=False).stdout.strip() != "tag":
        return "annotated 태그가 아니다 — hint 태그는 본문이 곧 페이로드다"
    body = git("cat-file", "tag", tag, check=False).stdout.partition("\n\n")[2]
    probs = lint_body(body)
    if probs:
        return (f"린트 위반 {len(probs)}건(예: {probs[0]}) — `seal` 은 이 본문으로 "
                "태그를 만들지 않는다(=도구를 거치지 않았다)")
    return None


# 참조 파일 **부재**를 뜻하는 코드의 닫힌 목록. tripwire 다 — 새 부재 코드가 생기면 여기 등재를
# 강제해 "조용히 blocking 으로 떨어지는" 일도, "조용히 통과하는" 일도 막는다.
_ABSENT_CODES = frozenset({
    "HINT_EVIDENCE_MANIFEST_REF_ABSENT",
    "HINT_EVIDENCE_CERTIFICATE_REF_ABSENT",
})


def classify_evidence_problems(per_tag: list) -> tuple[list[str], list[str]]:
    """evidence-binding 판정의 **단일 권위**. 입력 [(tag, [(code, message), ...])] →
    (blocking_messages[], unverifiable_tags[]).

    ★ 판정은 **code 로만** 한다. 종전엔 렌더된 message 를 부분일치(`CODE in msg`)로 분류했는데,
    message 에는 footer 값이 그대로 박히므로 발행자가 footer 필드에 코드 문자열을 심어 등급을
    뒤집을 수 있었다(감사 심각도1-② · 주입 실증: blocking 1→0). code 는 검증기가 정하고
    발행자가 건드릴 수 없다.

      *_REF_ABSENT 만              → unverifiable ("여기서는 대조할 수 없다")
      그 외 문제                    → blocking

    ★ unverifiable 은 "증거가 틀렸다"가 아니라 **"이 체크아웃에 참조 파일이 없다"** 다. 증거
    (work-manifest)는 배포되지 않으므로 수신자 클론에서는 사실상 전량이 여기 떨어진다. blocking 과
    한 등급으로 묶으면 수신자 카탈로그가 통째로 경고가 되어 신호가 죽는다(plan_26082017 §10.4).
    대신 도구 미경유는 `unsealed_reason()` 이 **파일 없이** 잡으므로 검출력은 유지된다.

    ★ 2026-09-01: v1 빈티지(legacy_warn)·증거보전 드리프트(drift_warn) 두 등급은 **삭제**했다.
    둘 다 "footer 규약 도입 이전 태그를 소급 차단하지 않는다"는 하위호환 장치였는데, 과거 태그
    전량 초기화로 소급 대상이 0이 되었다. 남겨두면 집행되지 않는 코드가 판정 경로에 남는다.
    """
    blocking: list[str] = []
    unverifiable: list[str] = []
    for tag, ev_problems in per_tag:
        if not ev_problems:
            continue
        if all(code in _ABSENT_CODES for code, _ in ev_problems):
            unverifiable.append(tag)
            continue
        blocking.extend(msg for _, msg in ev_problems)
    return blocking, unverifiable


# ── 강행 발행 절단선 (2026-09-06 · plan_26090616 Q7/Q8 · 사용자 결정) ─────────────────────────
#
# hint 태그의 롤은 **토큰노믹스 정책**이다 — 수신자가 같은 탐색을 다시 태우지 않게 하는 것.
# 그 목적에서 보면 "정보가 적은 태그" 는 결함이 아니라 **적은 정보** 이고, 진짜 위협은
# **거짓 정보**(서빙 실패를 성공으로 위장한 배포)다. 그래서 절단선을 다시 긋는다:
#
#   차단  = 양성 검출 — 위조 · 드리프트 · 미봉인 · 3신호 모순 · PII 매치 · **선언되지 않은** 부재
#   통과  = 선언된 부재 — 페이로드 `missing[]` 에 사유코드로 **적혀 있는** 결손
#
# 핵심은 "부재를 봐준다" 가 아니라 **"부재를 적었는가"** 다. 적으면 수신자가 무엇을 모른 채
# 소비하는지 알고, 안 적으면 그것이 곧 침묵이다. 인증서 발행은 벤치마커의 책임이고 캠페인 종료와
# hint 발행은 독립 사건이므로, 남의 책임 부재로 자기 발행을 막지 않는다.
_MISSING_CODE_FOR_ABSENCE = {
    "HINT_EVIDENCE_CERTIFICATE_REF_ABSENT": "HINT_MISSING_CERTIFICATE",
    # work-manifest 는 **배포되지 않는다**. 발행자 평면에서는 있어야 하므로 매핑하지 않는다 —
    # 여기서 면제하면 "증거 없이 발행" 이 아니라 "증거를 잃고도 발행" 이 통과한다.
}


def declared_missing_of(tag: str) -> frozenset:
    """태그 페이로드가 **스스로 선언한** 결손 사유코드. 읽을 수 없으면 빈 집합(면제 없음)."""
    data = _git_blob_bytes(f"{tag}^{{}}", "PAYLOAD.json")
    if not data:
        return frozenset()
    try:
        doc = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return frozenset()
    missing = doc.get("missing") if isinstance(doc, dict) else None
    return frozenset(m for m in (missing or []) if isinstance(m, str))


def absence_is_declared(tag: str, codes: list) -> bool:
    """이 태그의 `unverifiable` 사유가 **전부** 페이로드에 선언돼 있는가."""
    declared = declared_missing_of(tag)
    mapped = [_MISSING_CODE_FOR_ABSENCE.get(c) for c in codes]
    return bool(mapped) and all(m is not None and m in declared for m in mapped)


def _require_all_hint_tags_evidence_valid(action: str, tags: list | None = None) -> None:
    """모든 hint 태그의 evidence-binding 검증 — **계약 v2 분류**(hints/HINT_ISSUANCE_CONTRACT.md §5).

    v1 은 missing/forged/drifted 를 한 덩어리로 차단했다. 그 결과 evidence-binding footer 규약이
    Phase-3 에서 **나중에** 추가되면서, 그 이전 태그 23개가 이후의 모든 push 를 영구히 막았다.
    hint 태그의 목적은 토큰 이코노미이고(계약 §1), **정보가 적은 구버전은 결함이 아니다**.
    진짜 위협은 "서빙 실패를 성공으로 위장한 허위 배포"이지 형식 미비가 아니다(계약 §2).

    v2 분류(2026-09-04 현행으로 정정 — 감사 C-4: 종전 문구는 "footer 부재 태그의 빈티지 등급" 을
    약속했으나 그 코드는 2026-09-01 에 삭제됐다. 문서가 없는 완화를 약속하면 다음 스키마 변경에서
    같은 전수 차단이 재발한다):
      - footer 부재 / 위조 / 드리프트 / 미봉인 → **차단**
      - 폐기된 footer 키(`RETIRED_FOOTER_KEYS`)는 **어떤 태그에서도 차단된다** — 그 키를 면제하던
        빈티지 핀은 CP7.5(2026-09-04)에서 유일 항목의 태그가 폐기되며 함께 사라졌다
      - footer 의 certificate_ref 가 manifest 와 갈려도 **같은 측정**이면 통과(REBOUND · 앵커 트리
        판독) — 태그는 불변이고 사본 정리는 manifest 쪽에서 일어난다. 어디서도 읽을 수 없으면
        absent 이며 **통과가 아니다**
      - 참조 증거 부재 → unverifiable(수신자 평면 경고) — 단 **발행자 평면(verify·push)에서는 차단**
    """
    targets = tags if tags is not None else existing_hint_tags()
    per_tag = [(t, _validate_hint_tag_evidence(t, action)[1]) for t in targets]
    blocking, unverifiable = classify_evidence_problems(per_tag)
    # 배포 평면도 발행자 평면이다 — 부재/미봉인은 여기서 차단한다(사용자 결정 α의 경계).
    # 단 2026-09-06 부터 **선언된 부재는 예외**다(강행 발행 절단선 · 바로 위 주석).
    codes_of = {tag: [c for c, _ in probs] for tag, probs in per_tag}
    blocking = list(blocking)
    for x in unverifiable:
        if absence_is_declared(x, codes_of.get(x, [])):
            print(f"[hint_tag] NOTE {x}: 선언된 결손이라 차단하지 않는다 "
                  f"({', '.join(sorted(declared_missing_of(x)))}) — 강행 발행 정책", file=sys.stderr)
            continue
        blocking.append(f"{x}: HINT_EVIDENCE_REF_ABSENT 참조 증거가 이 체크아웃에 없고 "
                        "페이로드에 결손으로 **선언되지도 않았다** — 적히지 않은 부재는 침묵이다")
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
        print("[hint_tag] DRY-RUN (배포 없음). 실제 배포는 --apply.")
        git_push_authenticated(a.remote, refspec, dry_run=True)
        return 0
    git_push_authenticated(a.remote, refspec, dry_run=False)
    print("[hint_tag] pushed refs/tags/hint/* (last-good-* 미포함).")
    return 0


# ── match (근-미스 발견) ──────────────────────────────────────────────────────
def cmd_recipe_segment(a: argparse.Namespace) -> int:
    """인증서 → 레시피 세그먼트. 발행자가 이름을 짓지 못하게 하는 것이 목적이다(`derive_slug` 와 동형).

    read-only 이며 태그를 만들지 않는다 — 사람/에이전트가 이 값을 받아 이름을 조립한다.
    """
    path = Path(a.certificate)
    if not path.is_absolute():
        path = ROOT / path
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as e:
        die(f"[hint_tag] FAIL: 인증서를 읽을 수 없다({a.certificate}): {e}")
    fields, ok = _cgate().parse_flat_certificate(text)
    if not ok:
        die(f"[hint_tag] FAIL: flat 인증서로 파싱되지 않는다: {a.certificate}")
    print(derive_recipe_segment(fields))
    return 0


def cmd_match(a: argparse.Namespace) -> int:
    """근-미스 발견. **모델 비교는 정규화 + family 해소 후**에 한다 — 2026-08-20 실측에서
    `--model gemma-4-e2b-it` 검색이 정확일치인 `gemma-4-E2B-it/gb10` 을 '다른 모델'로 판정했다.
    그리고 모델 불일치분은 기본 숨긴다(출력 9,563 B 중 46/49 가 잡음이었다)."""
    idx, fam = _load_index(), _load_families()
    _, slugs = resolve_family(a.model, fam)
    nslugs = {_norm_slug(s) for s in slugs} | {_norm_slug(a.model)}
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
    # ── 2026-09-01 (plan_26090107 D1.1 · Phase 4): 손저작 카탈로그 평면 폐쇄.
    # 카탈로그의 진실원천은 이제 **원격 발행 태그**이고 파생기는 `hint_catalog.py derive` 다.
    # 이 명령이 살아 있으면 로컬 상태를 신뢰해 다시 쓰게 되고, 그것이 감사 ④(무검증 `active`
    # 기입 · 격리 세탁)와 ⑤(손저작 brief 오염 → 렌더 삼킴 18행)의 기전이었다.
    # **경로를 우회하지 않고 막는다** — D3 법칙: 정식 경로가 막히면 경로를 고친다.
    die("[hint_tag] 이 명령은 폐쇄됐다(plan_26090107 D1.1).\n"
        "  카탈로그는 손으로 쓰지 않는다 — 원격 발행 태그에서 파생한다:\n"
        "    python3 .claude/skills/hint-publisher/scripts/hint_catalog.py derive \\\n"
        "        --remote <원격> --generated-kst <YYYY-MM-DDTHH:MM:SS>\n"
        "  발행 자체는 `seal` 이 그대로 소유한다(태그 생성). 이 명령이 하던 색인 갱신만 옮겼다.")
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
    vllm, model, arch, recipe = parse_hint_tag(a.tag)
    fm = dict(re.findall(r"^(\w+):\s*(.+)$", body, re.M))
    topology = a.topology or fm.get("topology", "")
    anchor = fm.get("anchor") or git("rev-list", "-n", "1", a.tag).stdout.strip()
    if not topology:
        die("[hint_tag] FAIL: 태그 footer 에 topology 가 없고 --topology 도 미지정.")
    idx = _load_index()
    idx["hints"] = [e for e in idx["hints"] if e["tag"] != a.tag]
    idx["hints"].append({
        "tag": a.tag, "vllm": vllm, "model": model, "arch": arch, "recipe": recipe,
        "topology": topology, "brief": _brief_of(body), "anchor": anchor,
        "related": a.related or "", "status": "active",
        "last_verified": date.today().isoformat(),
    })
    idx["hints"].sort(key=lambda e: e["tag"])
    _save_index(idx)
    _wrote_md = _hints_regen(idx["hints"])
    print(f"[hint_tag] index.json{' + HINTS.md' if _wrote_md else ''} 갱신: {a.tag}"
          + ("" if _wrote_md else "  (HINTS.md 는 갱신되지 않았다 — 위 경고 참조)"))
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
        # ★ 침묵 배제 금지(2026-09-01 · 감사 ⑩). 종전엔 `.get("enabled")` 가 falsy 면 뺐는데,
        # `None`(= 판정 대기)과 `False`(= 실제로 끔)가 같은 취급을 받아 **SD 를 쓴 레시피가
        # 조용히 사라졌다**(qwen3.8-27b: 본문에 "MTP n=3 … 2.18×" 가 있는데 누락). 필터는
        # `True` 만 남기되, 판정 대기로 빠진 건수를 **반드시 알린다** — 빠진 줄 모르면
        # "없다"와 "모른다"가 구분되지 않는다.
        _sd = fam.get("tag_sd", {})
        _pending = [e["tag"] for e in rows
                    if (_sd.get(e["tag"], {}) or {}).get("enabled") is None]
        rows = [e for e in rows if (_sd.get(e["tag"], {}) or {}).get("enabled") is True]
        if _pending:
            print(f"[hint_tag] ⚠ --sd-only 로 {len(_pending)}건을 **판정 대기**라 뺐다 "
                  f"(SD 사용 여부 미상 — 없다는 뜻이 아니다). 본문 확인이 필요한 태그:",
                  file=sys.stderr)
            for _t in _pending:
                print(f"    - {_t}", file=sys.stderr)
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
    vllm, model, arch, recipe = parse_hint_tag(a.tag)
    _require_hint_promotion_target("hint_reverify", manifest, tag=a.tag,
                                    topology=entry.get("topology", ""), anchor=entry.get("anchor", ""),
                                    vllm=vllm, model=model)
    # blocker 2 (cycle 2) -- also validate the TARGET tag's own self-referenced evidence (not just
    # the supplied --manifest's binding to it) before mutating the index's last_verified stamp.
    _footer, ev_problems = _validate_hint_tag_evidence(a.tag, "hint_reverify")
    # 단일 대상이지만 분류는 동일 권위를 쓴다 — v1 빈티지 태그의 currency 스탬프 갱신까지
    # 막을 이유가 없다(계약 §4). forged/drifted 는 여기서도 그대로 차단된다.
    _blocking, _unver = classify_evidence_problems([(a.tag, ev_problems)])
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
    # ── 2026-09-01 (plan_26090107 D1.1 · Phase 4): 손저작 카탈로그 평면 폐쇄.
    # 카탈로그의 진실원천은 이제 **원격 발행 태그**이고 파생기는 `hint_catalog.py derive` 다.
    # 이 명령이 살아 있으면 로컬 상태를 신뢰해 다시 쓰게 되고, 그것이 감사 ④(무검증 `active`
    # 기입 · 격리 세탁)와 ⑤(손저작 brief 오염 → 렌더 삼킴 18행)의 기전이었다.
    # **경로를 우회하지 않고 막는다** — D3 법칙: 정식 경로가 막히면 경로를 고친다.
    die("[hint_tag] 이 명령은 폐쇄됐다(plan_26090107 D1.1).\n"
        "  카탈로그는 손으로 쓰지 않는다 — 원격 발행 태그에서 파생한다:\n"
        "    python3 .claude/skills/hint-publisher/scripts/hint_catalog.py derive \\\n"
        "        --remote <원격> --generated-kst <YYYY-MM-DDTHH:MM:SS>\n"
        "  발행 자체는 `seal` 이 그대로 소유한다(태그 생성). 이 명령이 하던 색인 갱신만 옮겼다.")
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
    problems, unverifiable = classify_evidence_problems(per_tag)
    # 태그별 귀속 — 같은 단일 권위(classify_evidence_problems)를 태그 하나씩 다시 태운다.
    # 메시지 문자열을 파싱해 태그를 캐내지 않는다(포맷이 바뀌면 조용히 어긋난다).
    unbound_why: dict[str, str] = {}
    unver_why: dict[str, str] = {}
    for _t, _ev in per_tag:
        _b, _u = classify_evidence_problems([(_t, _ev)])
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
                      "its binding -- refusing (no partial rewrite):\n  " + "\n  ".join(problems)},
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
        vllm, model, arch, recipe = parse_hint_tag(t)
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
    _wrote_md = _hints_regen(hints)
    dropped = sorted(set(prev) - set(tags))
    print(f"[hint_tag] reindex: {len(hints)} 태그 → index.json{' + HINTS.md' if _wrote_md else ''} "
          f"재생성(currency 보존)."
          + ("" if _wrote_md else "  (HINTS.md 는 갱신되지 않았다 — 위 경고 참조)")
          + (f"  제거(태그없음): {dropped}" if dropped else ""))
    return 0


def cmd_self_test(_a=None) -> int:
    """결정론 자체검사 — 라이브 원격·태그 없이 도는 것만 담는다.

    **왜 지금 생겼나**: push 자격증명 배선이 통째로 없었는데(2026-09-04) 이 스크립트에는 자체검사
    자체가 없어서 **아무도 그 부재를 물어보지 않았다**. 배선을 넣으면서 물어보는 자리도 같이 만든다.
    """
    import tempfile
    checks: list[tuple[str, bool]] = []

    def ck(n: str, c) -> None:
        checks.append((n, bool(c)))

    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        envf = t / ".env"

        # ── push 토큰 판독
        ck("파일 없으면 None", read_push_token(env_file=t / "nope", environ={}) is None)
        envf.write_text("# 주석\nHF_TOKEN=hf_x\nGITHUB_TOKEN=ghp_FILE\n", encoding="utf-8")
        ck("envs/.env 에서 판독", read_push_token(env_file=envf, environ={}) == "ghp_FILE")
        ck("★환경변수가 파일을 이긴다",
           read_push_token(env_file=envf, environ={"GITHUB_TOKEN": "ghp_ENV"}) == "ghp_ENV")
        envf.write_text("GITHUB_TOKEN=\n", encoding="utf-8")
        ck("★음성대조 빈 값은 토큰이 아니다", read_push_token(env_file=envf, environ={}) is None)
        envf.write_text("#GITHUB_TOKEN=ghp_commented\n", encoding="utf-8")
        ck("★음성대조 주석 처리된 줄은 무시", read_push_token(env_file=envf, environ={}) is None)
        envf.write_text('GITHUB_TOKEN="ghp_Q"\n', encoding="utf-8")
        ck("따옴표 제거", read_push_token(env_file=envf, environ={}) == "ghp_Q")

        # ── credential helper argv — ★ 토큰이 argv 에 실리면 안 된다(ps·reflog 유출)
        args = push_credential_args()
        ck("상속 helper 를 먼저 비운다", args[:2] == ["-c", "credential.helper="])
        ck("우리 helper 가 환경변수에서 읽는다", PUSH_TOKEN_ENV in args[3] and "$" + PUSH_TOKEN_ENV in args[3])
        ck("★토큰 값이 argv 에 없다", not any("ghp_" in a for a in args))
        ck("helper 는 get 이외 연산에 응답하지 않는다", 'test "$1" = get' in args[3])

    # ── footer 빈티지 핀(폐기 키 하위호환) — 음성대조가 핵심이다
    _f = ("<!-- hint-evidence-binding:v1\n"
          "version: 1\ntag: hint/x/y/z\ntopology: single 1노드\n"
          "anchor: " + "a" * 40 + "\nmanifest_ref: m.json\ncertificate_ref: c.yaml\n%s-->\n")
    ck("정상 footer 파싱", _parse_evidence_footer(_f % "")["tag"] == "hint/x/y/z")
    legacy = _f % "manifest_ref_note: x\n".replace("manifest_ref_note", "manifest_sha256")
    ck("★핀 있으면 폐기 키를 읽고 버린다",
       "manifest_sha256" not in _parse_evidence_footer(legacy, retired_ok=RETIRED_FOOTER_KEYS))
    def _boom(b, r=frozenset()):
        try:
            _parse_evidence_footer(b, retired_ok=r)
        except HintEvidenceBindingError as e:
            return e.code
        return None
    ck("★음성대조 핀 없으면 폐기 키는 여전히 차단",
       _boom(legacy) == "HINT_EVIDENCE_BINDING_MALFORMED")
    ck("★음성대조 폐기 키가 아닌 미지 키는 핀이 있어도 차단",
       _boom(_f % "totally_unknown: x\n", RETIRED_FOOTER_KEYS) == "HINT_EVIDENCE_BINDING_MALFORMED")
    ck("★seal 이 폐기 키를 발행하는 경로는 없다",
       not (set(_FOOTER_FIELDS) & RETIRED_FOOTER_KEYS)
       and not (RETIRED_FOOTER_KEYS & set(_build_evidence_footer(
           {k: "v" for k in _FOOTER_FIELDS}).split())))
    # ★ 이름을 **파일 텍스트**에서 찾으면 이 주석 자체가 매칭돼 가드가 스스로 무력화된다.
    #   모듈 네임스페이스를 본다 — 상수가 실제로 없는지를 묻는 것이 목적이다.
    ck("★CP7.5: 폐기 키 면제(빈티지 핀) 상수가 존재하지 않는다",
       "LEGACY_FOOTER_TAG_PINS" not in globals() and "LEGACY_CERTIFICATE_REF_ALIASES" not in globals())

    # ── 인증서 참조 해소의 순수 부분(plan_26090410 P4)
    _c = ("schema_version: 1\nverdict: PASS\nmodel: m\ngpu_model: GB10\nvllm_version: 0.18.0\n"
          "quantization: mxfp4\ntopology: single\ntensor_parallel_size: 1\nmeasured_utc: \"2026-09-03T12:00:00Z\"\n")
    ck("바이트 → 측정 키", _certificate_key_from_bytes(_c.encode())[-1] == "2026-09-03T12:00:00Z")
    ck("★음성대조 measured_utc 없으면 식별 불가(None)",
       _certificate_key_from_bytes(_c.replace('measured_utc: "2026-09-03T12:00:00Z"\n', "").encode()) is None)
    ck("★음성대조 중첩 YAML 은 식별 불가", _certificate_key_from_bytes(b"a:\n  b: 1\n") is None)
    ck("빈 바이트는 None", _certificate_key_from_bytes(None) is None and _certificate_key_from_bytes(b"") is None)
    # 별칭 핀 경로는 CP7.5 에서 사라졌다. 남은 계약은 **판정 불가를 통과로 접지 않는 것**이다 —
    #   어디서도 읽을 수 없는 footer 는 "absent" 로 끝나며 absent 는 통과가 아니다.
    #   문서 문자열이 아니라 **실제 호출**로 확인한다(주석 검사는 행동을 증명하지 못한다).
    with tempfile.TemporaryDirectory() as _td:
        _v, _src, _ = _resolve_footer_certificate(
            "hint/none/none/none/none", "../benchmark/does-not-exist.yaml",
            Path(_td), "../benchmark/also-missing.yaml")
    # 통과-계열 판정(`same_file`/`same_measurement`)에 **절대 도달하지 않는다**는 것이 계약이다.
    #   구체 판정은 저장소 밖 경로면 `unsafe`, 안이면 `absent` 로 갈리며 둘 다 통과가 아니다.
    ck("★음성대조 읽을 수 없는 footer 는 통과 판정에 도달하지 않는다(별칭 폴백 없음)",
       _v not in ("same_file", "same_measurement") and _src != "alias")

    # ── 본문 린터(음성대조 포함)
    good = "\n".join([f"## {n}. x\n" + ("가" * 90) for n, _ in REQUIRED_SECTIONS]) + "\narch-invariant\n"
    ck("완전한 본문은 린트 통과", lint_body(good) == [])
    ck("★음성대조 절 누락 검출", any("L1" in p for p in lint_body(good.replace("## 7. x", "## 9. x"))))
    ck("★음성대조 전이등급 없으면 검출",
       any("L3" in p for p in lint_body(good.replace("arch-invariant", "그냥 참고"))))
    ck("★음성대조 밀도 부족 검출", any("L2" in p for p in lint_body(good.replace("가" * 90, "짧음"))))

    # ── PII 스캔(배포면 4종)
    ck("★PII 절대경로 검출", any("abs-op-path" in h for h in scan_text("경로 /mnt/fixture-nas/Model/x 참조", None)))
    # 픽스처 값은 **합성**이다(`spark-[0-9a-f]{3,}` 을 만족하는 아무 값). 운영자 실호스트명을 쓰면
    # 이 추적·배포 파일이 그 이름을 싣게 되고, 그것이 곧 우리가 막으려는 유출이다(2026-09-04 실측:
    # 배포면 스캔이 이 줄을 `term:` 으로 잡았다). 합성 값으로도 **generic 패턴 발화**는 동일하게
    # 증명되므로 시험 강도는 그대로다 — 면제 마커가 아니라 값 교체가 정답인 이유다.
    ck("★PII 호스트명 검출", any("spark-host" in h for h in scan_text("노드 spark-0f0f 에서", None)))
    ck("깨끗한 본문은 무검출", scan_text("GB10 2노드 TP=2 · 53.92 t/s", None) == [])

    # ── CP7: 5세그먼트 이름 + 파생 레시피 (plan_26090415 §7.5 M1) ────────────────
    ck("5세그먼트 이름을 해체한다",
       parse_hint_tag("hint/0.19.1/gpt-oss-120b/gb10-single/qmxfp4-len131072-kvfp8")
       == ("0.19.1", "gpt-oss-120b", "gb10-single", "qmxfp4-len131072-kvfp8"))
    ck("★음성대조 4세그먼트(구세대)는 거부", not TAG_SHAPE.match("hint/0.19.1/gpt-oss-120b/gb10"))
    ck("★음성대조 6세그먼트도 거부", not TAG_SHAPE.match("hint/a/b/c/d/e"))
    # ── arch 노드 축(2026-09-06 · plan_26090616 Q1/Q2) ──
    ck("arch 3형상이 문법을 만족한다",
       all(arch_violation(x) is None for x in
           ("gb10-main-sim-h100", "gb10-sub-sim-h100", "gb10x2-cluster-sim-h100")))
    ck("★음성대조 노드 축 없는 옛 이름은 **다른 사유코드**로 거부",
       arch_violation("gb10-sim-h100") == "HINT_ARCH_NODE_AXIS_ABSENT")
    ck("★음성대조 대문자·미지 축은 형태 위반",
       arch_violation("GB10-main-x") == "HINT_ARCH_SHAPE_VIOLATION"
       and arch_violation("gb10-node-x") == "HINT_ARCH_NODE_AXIS_ABSENT")
    ck("arch 해체가 축 3개를 준다",
       parse_arch_segment("gb10x2-cluster-sim-h100") == ("gb10x2", "cluster", "sim-h100"))
    ck("조립기가 문법을 스스로 검사한다",
       build_arch_segment("gb10", "sub", "sim-h100") == "gb10-sub-sim-h100")
    _built = None
    try:
        build_arch_segment("gb10", "worker", "x")
    except SystemExit as _e:
        _built = _e
    ck("★음성대조 미지 노드 축 조립은 거부", _built is not None)
    ck("모델 슬러그 인덱스는 그대로 [2](레시피를 맨 뒤에 붙인 이유)",
       "hint/0.19.1/gpt-oss-120b/gb10-single/q-len-kv".split("/")[2] == "gpt-oss-120b")
    ck("인증서 3축에서 파생한다",
       derive_recipe_segment({"quantization": "mxfp4", "max_model_len": "131072",
                              "kv_cache_dtype": "fp8"}) == "qmxfp4-len131072-kvfp8")
    ck("★결측 축은 토큰을 만들지 않는다(N/A 를 값으로 적지 않는다)",
       derive_recipe_segment({"quantization": "N/A", "max_model_len": "32768",
                              "kv_cache_dtype": ""}) == "len32768")
    ck("파생값은 레시피 형태를 만족한다",
       bool(RECIPE_SHAPE.match(derive_recipe_segment(
           {"quantization": "mxfp4", "max_model_len": "131072", "kv_cache_dtype": "fp8"}))))
    ck("★음성대조 손저작 형태(대문자·슬래시·선행 하이픈)는 거부",
       not RECIPE_SHAPE.match("QMXFP4") and not RECIPE_SHAPE.match("q/len")
       and not RECIPE_SHAPE.match("-leading"))
    ck("충돌 폴백은 `_<timestamp>` 를 붙인다",
       collide_suffix("hint/a/b/c/qx", "260904T0730Z") == "hint/a/b/c/qx_260904T0730Z")
    ck("★충돌 폴백을 벗기면 파생값과 대조된다(인증서 교차검증이 여전히 성립)",
       "qmxfp4-len131072-kvfp8_260904T0730Z".split("_")[0] == "qmxfp4-len131072-kvfp8")
    ck("인덱스 행이 recipe 칸을 싣는다",
       "| qmxfp4-len1-kvfp8 |" in _hints_row(
           {"tag": "hint/a/b/c/qmxfp4-len1-kvfp8", "vllm": "a", "model": "b", "arch": "c",
            "recipe": "qmxfp4-len1-kvfp8"}))

    bad = [n for n, ok in checks if not ok]
    for n, ok in checks:
        print(f"  {'ok  ' if ok else 'FAIL'} {n}")
    print(f"[hint_tag] self-test {len(checks) - len(bad)}/{len(checks)} " + ("PASS" if not bad else "FAIL"))
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(description="hint/<vllm>/<model>/<arch> 레시피-힌트 태그 관리")
    if "--self-test" in sys.argv[1:]:
        return cmd_self_test()
    sub = ap.add_subparsers(dest="cmd", required=True)

    c = sub.add_parser("create", help="태그 검증 + 레시피 스캐폴드(TODO 슬롯)")
    c.add_argument("--tag", required=True)
    c.add_argument("--commit", default="HEAD")
    c.add_argument("--topology", required=True, help="예: 'multi 2노드 TP2' | 'single 1노드'")
    c.add_argument("--brief", default="")
    c.add_argument("--related", default="")
    c.add_argument("--from-resolved", default="resolved.json")
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
    sl.add_argument("--hf-repo", help="정본 HF repo '<org>/<name>' — 슬러그가 여기서 파생된다(R1)")
    sl.add_argument("--model-path", help="HF 미등록 커스텀 모델의 서빙 경로(D2 예외)")
    sl.set_defaults(fn=cmd_finalize, no_index=True)

    ix = sub.add_parser("index", help="로컬 hint 태그를 index.json + HINTS.md 에 편입 (중앙 전용)")
    ix.add_argument("--tag", required=True)
    ix.add_argument("--topology", default="", help="미지정 시 태그 footer 에서 읽는다")
    ix.add_argument("--related", default="")
    ix.set_defaults(fn=cmd_index)

    v = sub.add_parser("verify", help="릴리즈 게이트(태그오브젝트/build_patches PII·인덱스 정합)")
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

    rs = sub.add_parser("recipe-segment",
                        help="인증서에서 레시피 세그먼트를 **파생**한다(손저작 방지) -- read-only")
    rs.add_argument("--certificate", required=True, help="flat 인증서 YAML 경로")
    # ★ `func=` 오타로 이 서브커맨드는 **도달 불가**였다(디스패처는 `fn` 만 읽는다) — 파서는
    #   등록됐으므로 `--help` 에는 보이는데 실행하면 AttributeError 로 죽었다. 게다가 seal 의
    #   실패 메시지가 바로 이 명령을 실행하라고 안내한다: 가드가 **죽은 문을 가리키고 있었다**
    #   (2026-09-06 hint 발행에서 첫 실증 — 5세그먼트 전환 뒤 아무도 이 경로를 안 밟았다).
    rs.set_defaults(fn=cmd_recipe_segment)

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

    a = ap.parse_args()
    if a.cmd != "match":
        require_git_repository()
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
