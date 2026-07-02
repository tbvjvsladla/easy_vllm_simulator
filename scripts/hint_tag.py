#!/usr/bin/env python3
"""hint_tag.py — manage hint/<vllm>/<model>/<arch> recipe-hint tags.

Design: docs/plan/plan_2026070222_1. The deployment "hint" layer distributes
distilled serving-recipe KNOWLEDGE as annotated git tags — never finished products.

[A]=B residence: the recipe body lives in the tag ANNOTATION. HEAD stays a pure
skeleton+engine (no recipe files at HEAD); HEAD carries only an index (README 부록 +
hints/index.json). Retrieved by `git fetch --tags` + `git show <tag>`.

Deterministic here (script) / judgment there (agent):
  - validates names (shape + `git check-ref-format`), one canonical model-slug/family,
  - scaffolds from resolved.json (surgical scalar extraction — resolved.json itself
    carries operator PII in some notes, so only clean fields are pulled),
  - PII-scans FAIL-CLOSED over the FULL recipe body AND the tagger identity that ships
    inside the pushed tag object (pii_terms.txt literals — single source shared with
    scan_forbidden_strings.py — plus generic private-IP/email/path/host patterns),
  - enforces the B1 backstop (absolute host-scoped numbers require a re-measure caveat),
  - tags, indexes (index.json + README row), verifies, and pushes ONLY refs/tags/hint/*
    (never --tags, which would leak local last-good-* rollback anchors to a public origin).
The agent authors the judgment slots (context / wall-map / serve-knob why / re-verify).

Subcommands: create · finalize · verify · push · match · reverify.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

TAG_SHAPE = re.compile(r"^hint/[^/]+/[^/]+/[^/]+$")
README_MARKER = "<!-- hint-index:rows -->"

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
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    if out.returncode != 0:
        die("[hint_tag] FAIL: git 레포 안에서 실행해야 합니다.")
    return Path(out.stdout.strip())


ROOT = repo_root()
PII_TERMS_FILE = ROOT / ".claude" / "pii_terms.txt"
INDEX_FILE = ROOT / "hints" / "index.json"
README_FILE = ROOT / "README.md"
TEMPLATE_FILE = ROOT / "scripts" / "templates" / "hint_recipe.template.md"
DRAFTS_DIR = ROOT / "hints" / ".drafts"


def git(*args: str, check: bool = True, env: dict | None = None) -> subprocess.CompletedProcess:
    out = subprocess.run(["git", *args], capture_output=True, text=True, cwd=str(ROOT), env=env)
    if check and out.returncode != 0:
        die(f"[hint_tag] git {' '.join(args)} FAILED:\n{out.stderr.strip()}")
    return out


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


# ── create ──────────────────────────────────────────────────────────────────
def cmd_create(a: argparse.Namespace) -> int:
    vllm, model, arch = validate_name(a.tag, a.allow_new_slug)
    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()

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
    print(f"[hint_tag] scaffold → {out.relative_to(ROOT)}  (앵커 {anchor[:12]})")
    print("[hint_tag] 다음: TODO(judgment) 슬롯을 채운 뒤:")
    print(f"           python3 scripts/hint_tag.py finalize --tag {a.tag} "
          f"--recipe {out.relative_to(ROOT)} --commit {anchor[:12]} --topology '{a.topology}'"
          + (f" --related '{a.related}'" if a.related else ""))
    return 0


# ── finalize ────────────────────────────────────────────────────────────────
RE_ABS_HOST = re.compile(r"\b\d{2,}\s?GiB\b|\b\d{9,}\b|gmu\s*0?\.\d")
RE_REMEASURE = re.compile(r"측정|재도출|재측정|비이식|re-?measure|measure")


def _assert_remeasure(body: str) -> None:
    if RE_ABS_HOST.search(body) and not RE_REMEASURE.search(body):
        die("[hint_tag] FAIL(B1 백스톱): 절대 호스트-스코프 숫자(KV GiB/bytes/gmu)가 있으나 "
            "재측정 한정자(측정/재도출/비이식)가 없음 — 슬롯4를 measurement-first 로 재작성.")


def cmd_finalize(a: argparse.Namespace) -> int:
    validate_name(a.tag, a.allow_new_slug)  # 미존재·정본·합법 재확인
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

    tname = a.tagger_name or git("config", "user.name", check=False).stdout.strip()
    temail = a.tagger_email or git("config", "user.email", check=False).stdout.strip()
    idhits = scan_text(f"{tname} {temail}", terms, skip_generic=frozenset({"email"}))
    if idhits:
        die("[hint_tag] FAIL(tagger PII): tagger 신원이 PII 를 흘림: " + ", ".join(idhits)
            + "\n  → --tagger-name/--tagger-email 로 clean 한 공개 신원을 지정하세요"
            " (배포 태그 오브젝트에 tagger 가 박힙니다).")

    anchor = git("rev-parse", "--verify", a.commit).stdout.strip()
    env = dict(os.environ, GIT_COMMITTER_NAME=tname, GIT_COMMITTER_EMAIL=temail)
    git("-c", f"user.name={tname}", "-c", f"user.email={temail}",
        "tag", "-a", a.tag, anchor, "-F", str(recipe), env=env)
    print(f"[hint_tag] tagged {a.tag} → {anchor[:12]}  (tagger {tname} <{temail}>)")

    brief = _brief_of(body)
    _, vllm, model, arch = a.tag.split("/")
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

    _readme_add_row(a.tag, vllm, model, arch, a.topology, a.related or "", brief)
    print("[hint_tag] index.json + README 부록 인덱스 갱신 완료.")
    return 0


def _readme_add_row(tag, vllm, model, arch, topo, related, brief) -> None:
    if not README_FILE.is_file():
        return
    lines = README_FILE.read_text(encoding="utf-8").splitlines()
    row_key = f"| `{tag}` |"
    row = (f"| `{tag}` | {vllm} | {model} | {arch} | {topo} | active | "
           f"{related or '—'} | {date.today().isoformat()} | {brief} |")
    out, added = [], False
    for ln in lines:
        if ln.startswith(row_key):
            continue  # 같은 태그 기존 행 제거(멱등 — 재-finalize 시 중복 방지)
        if ln.strip() == README_MARKER and not added:
            out.append(row)
            added = True
        out.append(ln)
    if added:
        README_FILE.write_text("\n".join(out) + "\n", encoding="utf-8")


# ── verify ──────────────────────────────────────────────────────────────────
def cmd_verify(a: argparse.Namespace) -> int:
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


# ── push (선별) ──────────────────────────────────────────────────────────────
def cmd_push(a: argparse.Namespace) -> int:
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
    idx = _load_index()
    for e in idx["hints"]:
        if e["tag"] == a.tag:
            reachable = git("cat-file", "-e", e["anchor"] + "^{commit}", check=False).returncode == 0
            e["last_verified"] = date.today().isoformat()
            e["anchor_reachable"] = reachable
            _save_index(idx)
            print(f"[hint_tag] reverify {a.tag}: anchor_reachable={reachable} · last_verified 스탬프.")
            return 0
    die(f"[hint_tag] FAIL: {a.tag} 가 index 에 없음.")


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
    f.set_defaults(fn=cmd_finalize)

    v = sub.add_parser("verify", help="릴리즈 게이트(태그오브젝트/build_patches PII·인덱스 정합)")
    v.add_argument("--check-origin", action="store_true", help="origin 에 last-good-* 없음 확인(네트워크)")
    v.set_defaults(fn=cmd_verify)

    p = sub.add_parser("push", help="선별 배포 refs/tags/hint/* (--tags 금지)")
    p.add_argument("--remote", default="origin")
    p.add_argument("--apply", action="store_true")
    p.set_defaults(fn=cmd_push)

    m = sub.add_parser("match", help="근-미스 발견(축별 이식 가이드)")
    m.add_argument("--vllm", required=True)
    m.add_argument("--model", required=True)
    m.add_argument("--arch", required=True)
    m.set_defaults(fn=cmd_match)

    r = sub.add_parser("reverify", help="핀 자산 reachability + last_verified 스탬프")
    r.add_argument("--tag", required=True)
    r.set_defaults(fn=cmd_reverify)

    args = ap.parse_args()
    return args.fn(args)


if __name__ == "__main__":
    raise SystemExit(main())
