"""hintlib.branch — hint 브랜치 배관 커밋 (plan_26092119 §4.9 · SPEC §5.8 · 옛 `hint_branch.py` 이관).

무엇을 하나
    발행 draft 의 `payload/` 디렉터리 → allowlist 트리 → **안내 커밋을 부모로 한** 페이로드 커밋(오브젝트만 · ref 이동 ✗).
    태그는 만들지 않는다(봉인은 `tag.py` 소관). hint 태그가 가리키는 커밋의 **트리가 곧 배포물**(archive = zip)이므로, 이
    모듈은 "무엇이 배포되는가"의 단일 소유자다.

★ 브랜치 모델 v7 (2026-09-29 · plan_26092908 §4.7 · U8 · V12)
    옛 모델(페이로드 커밋 부모 = 로컬 hint tip · 브랜치 한 칸 전진)은 PC 마다 체인이 갈라졌고, 다른 PC 태그를 이 PC verify 가
    막았으며, 브랜치를 push 하면 원격 첫 화면이 마지막 셀 README 가 됐다. 이제:
      - 부모 = **안내 커밋**(원격 `refs/heads/hint` tip · ls-remote 읽기 · 트리 = README.md 하나 · 첫 줄 형식 마커
        `<!-- hint-branch-format: v7 -->`). 형식 불일치(마커 없는 옛 안내 포함) = HINT_BRANCH_FORMAT_MISMATCH → 형식 전환 명령.
      - 페이로드 커밋끼리 체인 ✗ · 로컬 `refs/heads/hint` 는 발행이 움직이지 않는다(재개 = draft state 의 anchor + 결정론 SHA).
      - 브랜치 push 는 `branch_transition`(사람 승인 · exact refspec non-force)뿐이다.

★ 워킹트리 0 설계 (plan D1.2 개정 · 2026-09-01 · 옛 hint_branch 에서 그대로)
    `git worktree` + 빈 인덱스 대신 **순수 배관**(hash-object → mktree → commit-tree → update-ref)만 쓴다.
    워킹트리·인덱스가 아예 없으므로
      C1 잔존물 오염      → 트리를 매번 allowlist 로 **새로** 짓는다. 넣지 않은 것은 들어갈 수 없다.
      C2 브랜치 전환 파괴 → 체크아웃 0회. 메인 워킹트리·인덱스는 관측조차 되지 않는다.
      C3 dirty clear 소실 → 지우는 동작 자체가 없다.
    세 위험이 가드로 막히는 것이 아니라 **구조적으로 불가능**하다. "성공을 선언하지 않고 결과를 검사한다"(C4)는
    유지한다 — 지은 트리를 다시 읽어 목록·모드·blob 이 입력과 같은지 본다.

★ 합성 신원 · 주입 시각 (2026-09-21 코드맵 K1·K2 · H10)
    옛 `commit-tree` 는 신원·날짜 env 없이 불려 **페이로드 커밋 57/57 이 운영자 실명·회사 이메일**로 찍혔고,
    태그가 그 커밋을 가리키므로 zip 을 받는 모든 수신자에게 배포됐다(4종 PII 스캔은 커밋 메타를 보지 않았다).
    커밋 시각도 벽시계라 같은 입력으로 다시 지으면 SHA 가 달랐다. 이제 author·committer 는 `core.SYNTHETIC_*`
    (tagger 와 같은 한 상수) · 날짜는 `--generated-utc` 주입값이다 → 같은 입력 = 같은 커밋 SHA.
    실명 override 경로는 두지 않는다(2026-09-01 tagger 실명 유출 선례 — "잊으면 새는" 구조를 만들지 않는다).

★ README 는 페이로드 생성기가 만든다 (K8)
    옛 publish 는 브랜치 tip 의 README 를 매 발행 페이로드에 심고, tip 에 없으면 죽었다(57/57 커밋이 같은 blob).
    형식 버전별 README 는 템플릿이 렌더하고 allowlist 필수 파일로 여기서 **검사만** 한다(부재 = 차단 ·
    plan §12 A6 "경고 README 는 잊을 수 없어야 한다" 는 필수 목록으로 유지된다).

앵커 3중 기록의 새 뜻 (코드맵 K3)
    옛 "ⓐ 커밋 메시지 · ⓑ footer · ⓒ PROVENANCE 가 같은 앵커" 는 실물에서 성립하지 않았다(footer 의 anchor 는
    **페이로드 커밋**, ⓐⓒ 는 **소스 커밋**). 이제 두 쌍으로 나눠 실제로 검사한다:
      ⓐ↔ⓒ  커밋 메시지 ⊇ PROVENANCE.source_anchor  (여기 `commit_payload` 와 `tag.verify_local`)
      ⓑ     footer.anchor == 태그가 가리키는 페이로드 커밋  (`tag.verify_local` · 옛날엔 한 번도 검사되지 않았다)

디스크 검사와 git 검사는 한 규칙이다 (2026-09-22)
    커밋 전(`commit_payload` · 디스크의 draft)과 봉인 전후(`commit_violations` · git 오브젝트)는 같은 페이로드 계약을
    묻는다. 두 자리에 규칙을 따로 적으면 갈라진다(workflow.md "개념 중복") — 문서 정합은 `payload_doc_problems` 한
    함수가 판정하고, 두 경로는 그 입력(바이트)을 어디서 읽는지만 다르다.
"""
from __future__ import annotations

import contextlib
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

from . import core, naming, pii

# ── 상수 ─────────────────────────────────────────────────────────────────────────────────────
# hint 브랜치 ref 는 한 자리(core)만 적는다 — 옛 hint_branch.HINT_BRANCH · hint_tag.HINT_BRANCH_REF 2벌(코드맵 D6).
HINT_BRANCH_REF = core.HINT_BRANCH_REF

PAYLOAD_JSON = "PAYLOAD.json"
PROVENANCE_JSON = "PROVENANCE.json"
ARTIFACTS_DIR = "artifacts"

# 페이로드 트리 allowlist (SPEC §2 · 형식 hint-payload/v6). 닫힌 목록이다 — workflow.md 안티패턴 판정표의
# "하드코딩의 정당 형태 = tripwire": 형식을 바꾸려면 이 줄을 고쳐야 하고 그때 리뷰가 강제된다.
PAYLOAD_ALLOWLIST_TOP = ("README.md", "00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md",
                         PAYLOAD_JSON, "LINEAGE.json", PROVENANCE_JSON)
# v6 형식에서 최상위 8종은 전부 필수다(README 부재 = 경고문 없는 배포 · PAYLOAD/PROVENANCE 부재 = 앵커 게이트가
# 뒤늦게 죽는다). 필수 목록을 allowlist 와 따로 적지 않는다 — 같은 개념을 두 자리에 쓰면 갈라진다.
PAYLOAD_REQUIRED_TOP = PAYLOAD_ALLOWLIST_TOP
# artifacts/<slot>/** 의 slot 어휘. ★ `artifacts.SLOTS` 와 같은 개념이다 — 정적 목록끼리는 한쪽이 다른 쪽을
#   생성할 수 없으므로 자체검사가 교차검증한다(workflow.md "단일 소유 불가 → 교차검증이 차선").
PAYLOAD_SLOT_DIRS = ("triplet", "runtime_patch", "build_patch_pre", "build_patch_post", "build_recipe",
                     "compose", "fork_pin")
# 페이로드 어디에도 들어가면 안 되는 이름(합격기준 A1 — archive 에 프로젝트 코드 0). 옛 FORBIDDEN_TOP 은
# **최상위 세그먼트만** 봐서 `artifacts/x/.git/...` 같은 중첩이 통과했다(코드맵 §1.1) → 모든 세그먼트를 본다.
# ★ 2026-09-22: `.gitattributes`·`.gitmodules` 추가 — `git archive`(= 수신자의 zip)는 **아카이브하는 트리 안의**
#   `.gitattributes` 를 따른다. 슬롯 안에 `export-ignore` 한 줄이 있으면 verify 가 검사한 트리와 수신자가 받는 zip 이
#   갈린다(격리 저장소 실측: 파일 1개가 zip 에서 사라졌다 · `export-subst` 는 내용을 바꾼다). AC3 "zip 만으로" 의 전제다.
FORBIDDEN_SEGMENTS = frozenset({".git", ".claude", "__pycache__", ".gitattributes", ".gitmodules"})
# 신 형식 표지(SPEC §2 · PAYLOAD.json `format`). 옛 형식 페이로드를 이 브랜치에 새로 얹지 않는다(P1: 옛 것은 읽기 전용).
# 2026-09-29 v6 → v7(plan_26092908 §4.2~§4.6 · D 통합 결정: template.FORMAT · 안내 README 형식 줄과 같은 값 — 교차검증은 hint.py 자체검사).
PAYLOAD_FORMAT = "hint-payload/v7"
# 사람·Agent 가 읽는 서사 4종. 봉인 전 `template.seal_prompts` 가 기재 지시(PROMPT)를 질문 한 줄로 바꾼다 —
# 지시가 남은 채 배포되면 수신자는 **저작되지 않은 빈칸**을 지도로 받는다. 커밋 전에 막는다: 페이로드 커밋은 결정론
# 오브젝트라 같은 입력이면 다시 지어도 같은 SHA 이고, 태그가 가리키는 순간 불변이다 — 잔재는 오브젝트 전에 막는다.
PAYLOAD_DOCS = ("00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md")
PROMPT_RESIDUE_RE = re.compile(r"<!--\s*PROMPT\b")
# 미저작 자리표시(`<<AGENT: …>>`)도 같은 "저작되지 않은 빈칸" 이다. 표지 문자열의 소유자는 template(AGENT_MARK)이고
# 여기서는 지연 import 로 읽는다(두 자리에 적지 않는다 · workflow.md 개념 중복). seal_prompts 가 이미 막지만, 커밋은
# seal_prompts 를 거쳤는지 묻지 않으므로 커밋 앞에서 한 번 더 막는다.

_FULL_SHA_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
_IDENT_LINE_RE = re.compile(r"^(author|committer|tagger) (.*) (\d+) ([+-]\d{4})$")
_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")


def _log(msg: str) -> None:
    print(f"[hint] {msg}", file=sys.stderr)


# ── 신원 ─────────────────────────────────────────────────────────────────────────────────────
def synthetic_identity() -> str:
    """배포 오브젝트(페이로드 커밋 author/committer · 태그 tagger)에 박히는 유일한 신원 `이름 <메일>`."""
    return f"{core.SYNTHETIC_NAME} <{core.SYNTHETIC_EMAIL}>"


def identity_env(generated_utc: str) -> dict[str, str]:
    """git 이 신원·시각을 **ambient 설정이 아니라 이 값**에서 읽게 하는 env. 운영자 env 의 GIT_AUTHOR_* 도 덮는다."""
    d = core.git_date(generated_utc)
    return {"GIT_AUTHOR_NAME": core.SYNTHETIC_NAME, "GIT_AUTHOR_EMAIL": core.SYNTHETIC_EMAIL,
            "GIT_AUTHOR_DATE": d,
            "GIT_COMMITTER_NAME": core.SYNTHETIC_NAME, "GIT_COMMITTER_EMAIL": core.SYNTHETIC_EMAIL,
            "GIT_COMMITTER_DATE": d}


def parse_ident_headers(raw: str) -> dict[str, dict]:
    """커밋·태그 오브젝트 헤더의 author/committer/tagger 줄 → {role: {"ident": "이름 <메일>", "epoch", "tz"}}."""
    out: dict[str, dict] = {}
    head = raw.split("\n\n", 1)[0]
    for line in head.splitlines():
        m = _IDENT_LINE_RE.match(line)
        if m and m.group(1) not in out:
            out[m.group(1)] = {"ident": m.group(2), "epoch": int(m.group(3)), "tz": m.group(4)}
    return out


def read_object_text(repo: Path, kind: str, rev: str) -> str | None:
    """`git cat-file <kind> <rev>` 원문(UTF-8). 없으면 None. 텍스트 모드 로캘 인코딩을 거치지 않는다."""
    data = core.git_bytes(repo, "cat-file", kind, rev, check=False)
    return None if data is None else data.decode("utf-8", "replace")


def commit_identity(repo: Path, commit: str) -> dict[str, dict]:
    raw = read_object_text(repo, "commit", commit)
    if raw is None:
        core.fail("HINT_COMMIT_UNREADABLE", f"커밋 오브젝트를 읽을 수 없다: {commit}")
    return parse_ident_headers(raw)


# ── PII (배포면 = 4종 전부 fail-closed · plan R1) ──────────────────────────────────────────────
# 리터럴 목록 부재 = 차단은 `pii.require_terms` 한 곳이 정한다(감사 ⑫ · 코드맵 K11 — 옛 hint_branch 는 die ·
# hint_collect 는 결손 기재로 의미가 반대였다). 여기서 다시 구현하지 않는다.


def _fmt_hit(where: str, h) -> str:
    return f"{where}:{h.line}: {h.pattern}:{str(h.match)[:48]}"


def scan_blobs(files, terms: list[str], *, exempt_log: list | None = None) -> list[str]:
    """(상대경로, bytes) 열 → 배포 프로필(4종+리터럴) 적중 목록. **git blob** 용(`commit_violations` → 봉인 전후 검증).
    디스크의 페이로드는 `pii.scan_tree(rels=…)` 가 같은 규칙으로 본다 — 두 경로의 의미가 같아야 한다:

    - 바이너리는 **내용으로** 판정해 건너뛴다(UTF-8 디코드 실패 · 감사 N-1: 확장자가 아니라 내용).
    - compose 의 셸 기본값(`${VAR:-<경로>}`)은 운영자 경로가 아니라 폴백이다(2026-09-01 · 옛 hint_branch
      `_SHELL_DEFAULT`) → 페이로드 트리에 한해 `shell_default_exempt=True`(= scan_tree 기본값). 실값은 env 로
      주입되고 그 env 는 배포되지 않는다(형상만 나간다). baking 된 실경로는 여전히 잡힌다. 면제는 pii 가 stderr 에
      `EXEMPT(shell-default)` 로 남긴다(조용히 넘기지 않는다).
    """
    hits: list[str] = []
    for rel, data in files:
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            continue
        for h in pii.scan_text(text, terms, profile="deploy", shell_default_exempt=True, exempt_log=exempt_log):
            hits.append(_fmt_hit(rel, h))
    return hits


def identity_pii_hits(terms: list[str], ident: str | None = None) -> list[str]:
    """신원 문자열의 PII(identity 프로필 = 배포 - email: 신원은 정의상 메일을 가진다).

    ★ 이 스캔만으로는 운영자 신원을 못 막는다 — 리터럴 목록에 없는 실제 주소는 통과한다(2026-09-01 tagger 실명
    유출이 그 형태). 막는 것은 합성 신원 **동일성** 검사(`tag.verify_local`)와 env 강제이고, 이것은 합성 상수가
    PII 로 바뀌는 사고의 tripwire 다."""
    who = ident if ident is not None else synthetic_identity()
    return [_fmt_hit("identity", h) for h in pii.scan_text(who, terms, profile="identity")]


# ── allowlist ────────────────────────────────────────────────────────────────────────────────
def allowlist_violation(rel: str) -> str | None:
    """페이로드 상대경로 하나의 판정. None = 허용. 순수 함수(파일·git 무관)."""
    if not isinstance(rel, str) or not rel:
        return "빈 경로"
    if rel.startswith("/") or "\\" in rel:
        return "절대경로·역슬래시"
    if _CONTROL_RE.search(rel):
        return "제어문자 포함"
    try:
        rel.encode("utf-8")
    except UnicodeEncodeError:
        # os.walk 는 UTF-8 이 아닌 이름을 surrogate 로 돌려준다 — 그 이름은 트리 오브젝트에 옮겨 적을 수 없고(배관 입력이
        # 깨진다), 수신 측 zip 에서도 다른 이름이 된다. 커밋 중 UnicodeEncodeError 로 새기 전에 판정으로 막는다.
        return "UTF-8 로 표현할 수 없는 이름"
    parts = rel.split("/")
    if any(p in ("", ".", "..") for p in parts):
        return "빈 세그먼트·상위탈출"
    bad = FORBIDDEN_SEGMENTS.intersection(parts)
    if bad:
        return f"금지 이름 {sorted(bad)}(합격기준 A1 — archive 에 프로젝트 코드 0)"
    if len(parts) == 1:
        return None if rel in PAYLOAD_ALLOWLIST_TOP else "최상위 allowlist 밖"
    if parts[0] == ARTIFACTS_DIR and len(parts) >= 3 and parts[1] in PAYLOAD_SLOT_DIRS:
        return None
    return f"{ARTIFACTS_DIR}/<{'|'.join(PAYLOAD_SLOT_DIRS)}>/** 밖"


def payload_files(payload_dir) -> list[str]:
    """페이로드 디렉터리의 파일 전수(정렬된 posix 상대경로). 목록 밖·심링크·비정규 파일·필수 부재 = 차단.

    allowlist 는 **실물에서** 계산한다 — 옛 `files.txt` 손목록은 선언이었고, 선언과 실물이 다르면 넣은 것이 아니라
    적은 것이 배포됐다. 심링크는 hash **이전에** 거부한다(옛 build_tree 는 hash-object 를 먼저 하고 모드를 나중에
    판정해 트리 밖 대상의 blob 이 object DB 에 먼저 써졌다 · 코드맵 §1.1).
    """
    root = Path(payload_dir)
    if not root.is_dir():
        core.fail("HINT_PAYLOAD_DIR_ABSENT", f"페이로드 디렉터리가 없다: {root}")
    links: list[str] = []
    odd: list[str] = []
    outside: list[str] = []
    rels: list[str] = []
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames.sort()
        for d in list(dirnames):
            p = Path(dirpath) / d
            if p.is_symlink():
                links.append(p.relative_to(root).as_posix())
                dirnames.remove(d)
        for f in sorted(filenames):
            p = Path(dirpath) / f
            rel = p.relative_to(root).as_posix()
            if p.is_symlink():
                links.append(rel)
                continue
            if not stat.S_ISREG(p.lstat().st_mode):
                odd.append(rel)
                continue
            why = allowlist_violation(rel)
            if why:
                outside.append(f"{rel} ({why})")
                continue
            rels.append(rel)
    if links:
        core.fail("HINT_PAYLOAD_SYMLINK", f"심링크는 페이로드에 담지 않는다(대상이 트리 밖일 수 있다): {links}")
    if odd:
        core.fail("HINT_PAYLOAD_NOT_REGULAR", f"정규 파일이 아닌 항목: {odd}")
    if outside:
        core.fail("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", f"allowlist 밖 파일 {len(outside)}건: {outside[:20]}",
                  f"페이로드에는 최상위 {list(PAYLOAD_ALLOWLIST_TOP)} 와 "
                  f"{ARTIFACTS_DIR}/<slot>/** 만 둔다(hintlib.branch.PAYLOAD_*).")
    missing = [t for t in PAYLOAD_REQUIRED_TOP if t not in rels]
    if missing:
        core.fail("HINT_PAYLOAD_REQUIRED_ABSENT", f"필수 최상위 파일 부재: {missing}",
                  "hint.py publish 가 스캐폴드를 만든다 — 지우지 않았는지 확인한다.")
    return sorted(rels)


# ── 트리 짓기 ────────────────────────────────────────────────────────────────────────────────
def _mode_of(p: Path) -> str:
    return "100755" if p.lstat().st_mode & stat.S_IXUSR else "100644"


def _hash_files(repo: Path, root: Path, rels: list[str]) -> dict[str, str]:
    """blob 을 object DB 에 쓴다. `--no-filters`: 저장소 .gitattributes(eol·lfs 등)가 배포 바이트를 바꾸지 않게 —
    페이로드는 디스크에 있는 바이트 그대로가 배포물이다."""
    out = core.git(repo, "hash-object", "-w", "--no-filters", "--stdin-paths",
                   input_text="".join(f"{root / r}\n" for r in rels)).stdout.split()
    if len(out) != len(rels):
        core.fail("HINT_TREE_BUILD_FAILED", f"hash-object 출력 {len(out)}건 ≠ 입력 {len(rels)}건")
    return dict(zip(rels, out))


def _mktree(repo: Path, entries: list[tuple[str, str, str, str]]) -> str:
    payload = "".join(f"{m} {t} {s}\t{n}\0" for m, t, s, n in entries)
    return core.git(repo, "mktree", "-z", input_text=payload).stdout.strip()


def build_tree(repo: Path, payload_dir, rels: list[str]) -> str:
    """allowlist 목록으로 트리를 **새로** 짓고, 지은 결과를 다시 읽어 입력과 대조한다(C4).

    ★ 2026-09-01 실물 회귀: 한 번의 for 로 접으면 접는 도중에 새 중간 디렉터리가 생겨 2단 이상 중첩에서 미완결이
      됐다(자체검사 픽스처가 1단뿐이라 못 봤다). 여기서는 중첩 사전을 재귀로 접는다 — 깊이에 무관하고, 자체검사가
      4단 중첩 회귀를 든다.
    """
    root = Path(payload_dir)
    for rel in rels:
        why = allowlist_violation(rel)
        if why:
            core.fail("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", f"{rel}: {why}")
        p = root / rel
        if p.is_symlink() or not p.is_file():
            core.fail("HINT_PAYLOAD_FILE_ABSENT", f"목록에 있으나 정규 파일 실물이 없다: {rel} — 부재를 성공으로 넘기지 않는다")
    shas = _hash_files(repo, root, rels)
    modes = {r: _mode_of(root / r) for r in rels}
    nested: dict = {}
    for rel in rels:
        node = nested
        *dirs, name = rel.split("/")
        for d in dirs:
            node = node.setdefault(d, {})
            if not isinstance(node, dict):
                core.fail("HINT_TREE_BUILD_FAILED", f"파일과 디렉터리 이름 충돌: {rel}")
        if name in node:
            core.fail("HINT_TREE_BUILD_FAILED", f"중복 경로: {rel}")
        node[name] = rel

    def fold(node: dict) -> str:
        entries = []
        for name, v in node.items():
            if isinstance(v, dict):
                entries.append(("040000", "tree", fold(v), name))
            else:
                entries.append((modes[v], "blob", shas[v], name))
        return _mktree(repo, entries)

    tree = fold(nested)
    got = {e["path"]: (e["mode"], e["sha"]) for e in tree_entries(repo, tree)}
    want = {r: (modes[r], shas[r]) for r in rels}
    if got != want:
        extra = sorted(set(got) - set(want))
        lack = sorted(set(want) - set(got))
        diff = sorted(k for k in set(got) & set(want) if got[k] != want[k])
        core.fail("HINT_TREE_BUILD_FAILED", f"트리 전수 대조 실패 — 초과 {extra} · 부족 {lack} · 모드/blob 불일치 {diff}")
    return tree


def tree_entries(repo: Path, rev: str) -> list[dict]:
    """`ls-tree -r -z` → [{mode, type, sha, path}] (blob·gitlink 만 · 트리는 펼친다)."""
    raw = core.git_bytes(repo, "ls-tree", "-r", "-z", "--full-tree", rev)
    out = []
    for rec in raw.split(b"\0"):
        if not rec:
            continue
        meta, _, path = rec.partition(b"\t")
        mode, typ, sha = meta.decode("ascii").split(" ")
        out.append({"mode": mode, "type": typ, "sha": sha, "path": path.decode("utf-8", "replace")})
    return out


def tree_paths(repo: Path, rev: str) -> list[str]:
    return sorted(e["path"] for e in tree_entries(repo, rev))


def tree_violations(repo: Path, rev: str) -> list[str]:
    """이미 있는 커밋·트리가 allowlist 계약을 지키는지(봉인 후 로컬 검증 · D10). `CODE: 메시지` 목록."""
    probs: list[str] = []
    paths = []
    for e in tree_entries(repo, rev):
        paths.append(e["path"])
        if e["type"] != "blob":
            probs.append(f"HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST: {e['path']} (blob 이 아닌 항목 {e['type']})")
            continue
        if e["mode"] == "120000":
            probs.append(f"HINT_PAYLOAD_SYMLINK: {e['path']}")
            continue
        why = allowlist_violation(e["path"])
        if why:
            probs.append(f"HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST: {e['path']} ({why})")
    missing = [t for t in PAYLOAD_REQUIRED_TOP if t not in paths]
    if missing:
        probs.append(f"HINT_PAYLOAD_REQUIRED_ABSENT: 필수 최상위 파일 부재 {missing}")
    return probs


# ── 페이로드 문서 정합 (PAYLOAD · PROVENANCE · 커밋 메시지) ─────────────────────────────────────
def _read_json_file(path: Path, code: str) -> dict:
    doc = core.read_json(path, code=code)
    if not isinstance(doc, dict):
        core.fail(code, f"JSON 객체가 아니다: {path.name}")
    return doc


def commit_message(payload_dir, *, generated_utc: str) -> str:
    """페이로드 커밋 메시지를 **파생**한다(D8 발행자 입력 ✗ · 코드맵 §1.3 O-br2). 서사는 00-hint.md 에 있으므로
    여기 다시 적지 않는다(한 사실은 한 곳에). 옛 메시지는 손저작 자유 형식이었다."""
    root = Path(payload_dir)
    payload = _read_json_file(root / PAYLOAD_JSON, "HINT_PAYLOAD_JSON_UNREADABLE")
    prov = _read_json_file(root / PROVENANCE_JSON, "HINT_PROVENANCE_UNREADABLE")
    sa = prov.get("source_anchor")
    return (f"hint: {payload.get('tag')}\n\n"
            f"source_anchor: {sa if sa else 'none'}\n"
            f"payload_format: {payload.get('format')}\n"
            f"generated_utc: {core.require_utc(generated_utc)}\n")


def payload_doc_problems(payload, prov, *, paths, message, docs: dict | None = None,
                         expect_tag: str | None = None) -> list[str]:
    """PAYLOAD.json · PROVENANCE.json · 커밋 메시지 · 서사 4종의 정합(순수 · 부작용 0). `CODE: 설명` 목록(빈 = 정합).

    디스크(`commit_payload`)와 git(`commit_violations`)이 **같은 함수**로 판정한다 — 입력 바이트의 출처만 다르다.
    paths = 페이로드 파일 전수(상대경로) · docs = {서사 파일명: bytes}(PROMPT 잔재 검사 · 부재 항목은 건너뛴다 —
    필수 부재는 allowlist 검사의 몫이다) · expect_tag = 봉인하려는 태그(있으면 PAYLOAD.tag 와 대조).
    순서가 곧 우선순위다: `commit_payload` 는 첫 항목의 code 로 실패한다(메시지에는 전부를 싣는다).

    ★ 2026-09-22 리뷰: 서사 잔재 검사는 PAYLOAD/PROVENANCE 판정의 조기 반환과 **무관하게** 늘 돈다. 종전에는
      PAYLOAD.json 이 `[]` 처럼 객체가 아닌 JSON 이면 git 쪽 검사가 문서 판정을 통째로 건너뛰어, PROMPT 잔재까지 실린
      손 배관 커밋이 위반 0 으로 읽혔다(격리 저장소 실측). 한 결함이 다른 검사를 끄지 않는다.
    """
    return _meta_problems(payload, prov, paths=paths, message=message, expect_tag=expect_tag) + doc_residue_problems(docs)


def _meta_problems(payload, prov, *, paths, message, expect_tag: str | None) -> list[str]:
    """payload_doc_problems 의 PAYLOAD·PROVENANCE·메시지·이름 부분(조기 반환 = 뒤 판정이 의미를 잃는 결함에서만)."""
    out: list[str] = []

    def add(code: str, msg: str) -> None:
        out.append(f"{code}: {msg}")

    if not isinstance(payload, dict):
        add("HINT_PAYLOAD_JSON_UNREADABLE", "PAYLOAD.json 이 JSON 객체가 아니다.")
        return out
    if not isinstance(prov, dict):
        add("HINT_PROVENANCE_UNREADABLE", "PROVENANCE.json 이 JSON 객체가 아니다.")
        return out
    tag = payload.get("tag")
    if not isinstance(tag, str) or not tag.startswith(core.HINT_TAG_PREFIX):
        add("HINT_PAYLOAD_TAG_ABSENT", f"PAYLOAD.tag 가 hint 태그가 아니다: {tag!r}")
        return out
    if expect_tag is not None and tag != expect_tag:
        add("HINT_PAYLOAD_TAG_MISMATCH", f"PAYLOAD.tag={tag!r} ≠ 봉인 대상 {expect_tag!r} — 남의 페이로드다.")
    if prov.get("tag") != tag:
        add("HINT_PAYLOAD_TAG_MISMATCH", f"PROVENANCE.tag={prov.get('tag')!r} ≠ PAYLOAD.tag={tag!r} — 남의 페이로드다.")
    # 신규 발행 이름은 v6 문법이어야 한다(D5~D8 · 이름은 도구가 파생한다). 옛 세대 이름은 읽기 전용이다(P1).
    try:
        naming.validate_new_name(tag)
    except core.HintError as e:
        add(e.code, e.message)
    if payload.get("format") != PAYLOAD_FORMAT:
        add("HINT_PAYLOAD_FORMAT", f"PAYLOAD.format={payload.get('format')!r} ≠ {PAYLOAD_FORMAT!r} — 이 브랜치에 새로 "
                                   "얹는 페이로드는 신 형식만이다.")
    declared = prov.get("payload_files")
    if not isinstance(declared, list) or not all(isinstance(x, str) for x in declared) \
            or len(set(declared)) != len(declared):
        add("HINT_PROVENANCE_SHAPE", "PROVENANCE.payload_files 는 중복 없는 문자열 목록이어야 한다.")
    elif set(declared) | {PROVENANCE_JSON} != set(paths):
        # 선언이 아니라 결과를 검사한다(C4). PROVENANCE 자신은 목록에 있어도 없어도 된다 — 자기참조라 쓰기 순서에
        # 따라 갈린다. 그 한 건 말고는 선언 = 실물이어야 한다.
        add("HINT_PROVENANCE_FILES_MISMATCH",
            f"PROVENANCE.payload_files ≠ 실물 — 선언만 {sorted(set(declared) - set(paths))} · "
            f"실물만 {sorted(set(paths) - set(declared) - {PROVENANCE_JSON})}")
    sa = None
    if "source_anchor" not in prov:
        add("HINT_PROVENANCE_SHAPE", "PROVENANCE.source_anchor 키가 없다(값이 없으면 null 로 명시한다).")
    else:
        sa = prov["source_anchor"]
        if sa is not None and (not isinstance(sa, str) or not _FULL_SHA_RE.match(sa)):
            add("HINT_PROVENANCE_SHAPE", f"PROVENANCE.source_anchor 가 전체 SHA 가 아니다: {sa!r}")
            sa = None
    if not isinstance(message, str) or not message.strip() or tag not in message:
        add("HINT_COMMIT_MESSAGE_SHAPE", "커밋 메시지는 비어 있지 않고 태그 이름을 담아야 한다"
                                         "(branch.commit_message 로 파생한다).")
    elif sa and sa not in message:
        # ⓐ↔ⓒ: 하나만 기록하면 그 하나가 틀렸을 때 대조할 상대가 없다(옛 verify_anchor_triple 의 뜻).
        add("HINT_ANCHOR_TRIPLE_MISMATCH", f"커밋 메시지에 PROVENANCE.source_anchor({sa[:12]})가 없다.")
    # 이름 재파생 대조(코드맵 §8.4 "이름은 파생 · 봉인 때 다시 파생해 대조한다"): PAYLOAD.naming 의 축에서 이름을 다시
    # 조립해 PAYLOAD.tag 와 같아야 한다. 대조할 사실이 없으면 대조하지 않은 것이지 통과한 것이 아니다.
    nm = payload.get("naming")
    if not isinstance(nm, dict):
        add("HINT_PAYLOAD_NAMING_ABSENT", "PAYLOAD.naming(축별 {값, 출처})이 없다 — 이름 재파생 대조가 불가능하다.")
    else:
        try:
            got = naming.compose_from_payload(nm)
            if got != tag:
                add("HINT_DERIVED_NAME_MISMATCH", f"PAYLOAD.naming 재조립 {got!r} ≠ PAYLOAD.tag {tag!r}")
        except core.HintError as e:
            add(e.code, e.message)
    return out


def doc_residue_problems(docs: dict | None) -> list[str]:
    """서사 4종의 저작 잔재(PROMPT 지시 · `<<AGENT:` 자리표시). docs = {파일명: bytes|str} · 부재 항목은 건너뛴다."""
    from . import template  # noqa: PLC0415 — 표지 소유자(AGENT_MARK) · import 부수효과 0
    out: list[str] = []
    for name in PAYLOAD_DOCS:
        data = (docs or {}).get(name)
        if data is None:
            continue
        text = data.decode("utf-8", "replace") if isinstance(data, bytes) else str(data)
        m = PROMPT_RESIDUE_RE.search(text)
        if m:
            out.append(f"HINT_PROMPT_RESIDUE: {name}:{text.count(chr(10), 0, m.start()) + 1}: 기재 지시(PROMPT)가 "
                       "남았다 — template.seal_prompts 전이거나 저작이 끝나지 않았다.")
        i = text.find(template.AGENT_MARK)
        if i >= 0:
            out.append(f"HINT_AGENT_PLACEHOLDER_RESIDUE: {name}:{text.count(chr(10), 0, i) + 1}: 미저작 자리표시 "
                       f"`{template.AGENT_MARK}` 가 남았다 — 그 절을 저작한 뒤 커밋한다.")
    return out


def _validate_payload_docs(root: Path, rels: list[str], message: str) -> str:
    """커밋 전 정합(부작용 0 · 디스크). 반환 = 페이로드가 말하는 태그. 판정은 `payload_doc_problems`."""
    payload = _read_json_file(root / PAYLOAD_JSON, "HINT_PAYLOAD_JSON_UNREADABLE")
    prov = _read_json_file(root / PROVENANCE_JSON, "HINT_PROVENANCE_UNREADABLE")
    docs = {n: (root / n).read_bytes() for n in PAYLOAD_DOCS if n in rels}
    probs = payload_doc_problems(payload, prov, paths=rels, message=message, docs=docs)
    if probs:
        code = probs[0].split(":", 1)[0]
        core.fail(code, f"페이로드 문서 정합 {len(probs)}건:\n  " + "\n  ".join(probs),
                  "hint.py publish 가 만든 스캐폴드를 손으로 고치지 않았는지 확인하고, 저작을 마친 뒤 hint.py continue 로 "
                  "다시 실행한다.")
    return str(payload["tag"])


# ── ref ───────────────────────────────────────────────────────────────────────────────────────
def hint_tip(repo: Path) -> str | None:
    """로컬 `refs/heads/hint`(형식 전환·자체검사 전용 — 발행 경로는 읽지 않는다 · §4.7)."""
    tip = core.git(repo, "rev-parse", "--verify", "--quiet", HINT_BRANCH_REF, check=False).stdout.strip()
    return tip or None


def _require_hint_not_checked_out(repo: Path) -> None:
    """hint 브랜치가 어느 워크트리에든 체크아웃돼 있으면 전진하지 않는다.

    update-ref 는 체크아웃 여부를 묻지 않는다 — 그 워크트리의 HEAD 가 가리키는 브랜치를 배관으로 옮기면 인덱스·
    워킹트리는 옛 tip 에 남아 **전 파일이 변경된 것처럼** 보인다(옛 C2 "브랜치 전환 파괴" 의 다른 입구). 워킹트리 0
    설계가 구조적으로 막는 것은 *우리가* 체크아웃하는 경로뿐이므로, 남이 체크아웃한 경우는 여기서 소리낸다."""
    out = core.git(repo, "worktree", "list", "--porcelain", check=False)
    if out.returncode != 0:
        core.fail("HINT_GIT_FAILED", f"git worktree list 실패(rc={out.returncode}): {out.stderr.strip()}")
    if any(ln.strip() == f"branch {HINT_BRANCH_REF}" for ln in out.stdout.splitlines()):
        core.fail("HINT_HINT_BRANCH_CHECKED_OUT",
                  f"{HINT_BRANCH_REF} 가 워크트리에 체크아웃돼 있다 — 배관으로 전진하면 그 워크트리가 깨진다.",
                  "그 워크트리를 다른 브랜치로 옮긴 뒤 다시 실행한다(hint 브랜치는 체크아웃하지 않는다).")


def _update_hint_ref(repo: Path, new: str, expected_old: str | None) -> None:
    """compare-and-swap. 브랜치가 없을 때도 old 를 생략하지 않고 **zero-oid 로 "생성 전용"** CAS 를 건다 —
    옛 publish 는 old 를 주지 않아 그 사이 누가 만든 브랜치를 덮을 수 있었다(코드맵 §1.2 11)."""
    old = expected_old if expected_old else "0" * len(new)
    r = core.git(repo, "update-ref", "-m", f"hint: branch {new[:12]}", HINT_BRANCH_REF, new, old, check=False)
    if r.returncode != 0:
        core.fail("HINT_BRANCH_CAS_CONFLICT",
                  f"{HINT_BRANCH_REF} 가 그 사이 바뀌었다(기대 {old[:12]}) — 덮지 않는다: {r.stderr.strip()}",
                  "다른 발행이 끼어들었다. 브랜치 상태를 확인하고 hint.py continue 를 다시 실행한다.")


# ── 안내 커밋 (plan_26092908 §4.7 · U8 · V12) ─────────────────────────────────────────────────────
# ★ 2026-09-29 V12: 옛 모델은 페이로드 커밋의 부모 = **로컬** hint tip 이었다 → PC 마다 체인이 갈라지고(다른 PC 태그는 이 PC
#   verify 에서 HINT_ANCHOR_NOT_ON_HINT_BRANCH) · 브랜치를 push 하면 원격 첫 화면이 마지막 셀 README 로 바뀌었다. 새 모델:
#   - 페이로드 커밋의 부모 = **안내 커밋**(원격 `refs/heads/hint` tip · ls-remote 로 **읽기만**). 페이로드 커밋끼리 체인을 만들지
#     않는다 → PC 간 분기가 원천적으로 없다. 페이로드 커밋은 브랜치에 얹지 않고 **태그만** 가리킨다(로컬 hint 불이동 · 재개
#     멱등성은 draft state 의 anchor 와 결정론 SHA 로 판단한다).
#   - 안내 커밋 = 트리가 README.md 하나(100644) ∧ README **첫 줄**이 형식 마커 `<!-- hint-branch-format: vN -->`. 마커는 발행기가
#     "이 브랜치가 어느 형식의 안내인가" 를 읽는 유일한 기계 표면이다(산문 문구로 판정하지 않는다).
#   - 형식 ≠ 발행기 형식(`BRANCH_FORMAT`) = HINT_BRANCH_FORMAT_MISMATCH. **마커가 없는 옛 안내**(v6 전환 커밋 `6294a91` 처럼
#     README 하나이지만 마커 도입 전)도 같은 코드로 막는다 — 형식을 선언하지 않은 안내는 v7 과 같다고 추정하지 않는다. 처방은
#     `hint.py branch-transition`(사람 승인 · G2)이 그 위에 v7 안내 커밋을 얹고 exact refspec 로 non-force push 하는 것뿐이다.
#   - 브랜치 push 는 형식 전환 때만(발행은 브랜치를 밀지 않는다 · 자동 push 권한 ✗).
BRANCH_FORMAT = "v7"
GUIDE_README = "README.md"
_GUIDE_MARKER_RE = re.compile(r"^<!-- hint-branch-format: (v[1-9][0-9]*) -->$")


def guide_marker(fmt: str = BRANCH_FORMAT) -> str:
    """안내 README 첫 줄(개행 없음). 템플릿 `templates/hint-branch-README.md`(D 소유) 첫 줄이 이 바이트여야 한다."""
    if not isinstance(fmt, str) or not re.fullmatch(r"v[1-9][0-9]*", fmt):
        core.fail("HINT_BRANCH_FORMAT_SHAPE", f"형식 버전 모양이 아니다: {fmt!r}")
    return f"<!-- hint-branch-format: {fmt} -->"


def readme_format(data) -> str | None:
    """README 바이트/문자열 → 첫 줄 형식 마커의 버전(`v7` …) · 마커 없음 = None(마커 도입 전 안내 · 일반 README)."""
    if data is None:
        return None
    text = data.decode("utf-8", "replace") if isinstance(data, bytes) else str(data)
    m = _GUIDE_MARKER_RE.match(text.split("\n", 1)[0].rstrip("\r"))
    return m.group(1) if m else None


def _format_num(fmt: str | None) -> int:
    return int(fmt[1:]) if fmt else 0


def _commit_parents(raw: str) -> list[str]:
    return [ln[7:] for ln in raw.split("\n\n", 1)[0].splitlines() if ln.startswith("parent ")]


def guide_info(repo: Path, sha: str) -> tuple[str | None, str | None]:
    """로컬 커밋 하나가 안내 커밋인가 → (형식 | None, 안내가 아닌 사유 | None). 읽기만 한다.
    (fmt, None) = 형식 마커를 가진 안내 · (None, 사유) = 커밋 아님 · 트리 모양 위반 · 마커 없음."""
    raw = read_object_text(repo, "commit", sha) if sha else None
    if raw is None:
        return None, f"커밋 오브젝트가 로컬에 없다({str(sha)[:12]})"
    ents = tree_entries(repo, sha)
    if len(ents) != 1 or ents[0]["path"] != GUIDE_README or ents[0]["type"] != "blob" or ents[0]["mode"] != "100644":
        return None, f"트리가 {GUIDE_README} 하나(100644)가 아니다 — 안내 커밋이 아니다(항목 {len(ents)}개)"
    fmt = readme_format(read_blob(repo, sha, GUIDE_README))
    if fmt is None:
        return None, "README 첫 줄에 형식 마커(`<!-- hint-branch-format: vN -->`)가 없다 — 마커 도입 전(v6 이하) 안내다"
    return fmt, None


def _format_mismatch(where: str, sha: str, fmt: str | None, why: str | None):
    core.fail("HINT_BRANCH_FORMAT_MISMATCH",
              f"{where} {str(sha)[:12]} 는 발행기 형식 {BRANCH_FORMAT} 의 안내 커밋이 아니다 — "
              + (why if why else f"안내 형식 {fmt} ≠ 발행기 형식 {BRANCH_FORMAT}"),
              "형식 전환이 필요하다: 사람 승인(G2) 뒤 `hint.py branch-transition --remote <원격> --generated-utc <UTC> "
              "--approved-by \"<사람 발화 전사>\" --approved-utc <UTC>` 가 그 위에 v7 안내 커밋을 얹고 exact refspec 로 "
              "non-force push 한다(발행은 브랜치를 밀지 않는다).")


def require_guide(repo: Path, sha: str, *, where: str = "안내 커밋") -> str:
    """sha 가 **발행기 형식**(BRANCH_FORMAT)의 안내 커밋인가(로컬 오브젝트 · 네트워크 ✗). 아니면 HINT_BRANCH_FORMAT_MISMATCH."""
    if not isinstance(sha, str) or not _FULL_SHA_RE.match(sha):
        core.fail("HINT_BRANCH_GUIDE_SHAPE", f"안내 커밋은 전체 SHA 여야 한다: {sha!r}")
    fmt, why = guide_info(repo, sha)
    if fmt != BRANCH_FORMAT:
        _format_mismatch(where, sha, fmt, why)
    return sha


def _ensure_local_commit(repo: Path, remote: str, sha: str) -> None:
    """원격 tip 오브젝트가 로컬에 없으면 그 **한 ref** 만 fetch 한다(원격 추적 ref 갱신 허용 · 로컬 브랜치 이동 ✗)."""
    if core.git(repo, "cat-file", "-e", f"{sha}^{{commit}}", check=False).returncode == 0:
        return
    from . import tag  # noqa: PLC0415 — tag → branch import 순환 회피
    tag.fetch_ref_objects(repo, remote, HINT_BRANCH_REF)
    if core.git(repo, "cat-file", "-e", f"{sha}^{{commit}}", check=False).returncode != 0:
        core.fail("HINT_BRANCH_GUIDE_UNREADABLE",
                  f"원격 {HINT_BRANCH_REF} tip {sha[:12]} 을 fetch 뒤에도 로컬에서 읽을 수 없다 — 그 사이 원격 tip 이 바뀌었을 수 있다.",
                  "다시 실행한다(조회와 fetch 사이 경합).")


def guide_commit(repo: Path, remote: str, *, pinned: str | None = None) -> tuple[str, str]:
    """페이로드 커밋의 부모가 될 안내 커밋 → (sha, 형식). 형식 ≠ BRANCH_FORMAT 이면 HINT_BRANCH_FORMAT_MISMATCH.

    기본: 원격 `refs/heads/hint` tip 을 `ls-remote` 로 **읽는다**(쓰기 ✗). 오브젝트가 로컬에 없으면 그 한 ref 만 fetch 한다.
    원격 조회 실패 = HINT_REMOTE_QUERY_FAILED(fail-closed · '없음' 으로 접지 않는다) · 원격에 hint 브랜치 없음 =
    HINT_BRANCH_GUIDE_ABSENT. 로컬 `refs/heads/hint` 는 읽지도 움직이지도 않는다.
    pinned: 네트워크 없이 **로컬 오브젝트**를 안내로 쓴다(자체검사·오프라인 재생 전용 — 원격 조회를 하지 않았다는 사실은
    호출자가 기록한다). 같은 판정(require_guide)을 받는다."""
    if pinned is not None:
        return require_guide(repo, pinned, where="지정 안내 커밋(pinned)"), BRANCH_FORMAT
    if not isinstance(remote, str) or not remote.strip() or remote.lstrip().startswith("-"):
        core.fail("HINT_BRANCH_REMOTE_INVALID", f"안내 커밋을 읽을 원격 인자가 올바르지 않다: {remote!r}")
    from . import tag  # noqa: PLC0415
    tip = tag.remote_ref_object(repo, remote, HINT_BRANCH_REF)
    if not tip:
        core.fail("HINT_BRANCH_GUIDE_ABSENT", f"원격 {remote!r} 에 {HINT_BRANCH_REF} 가 없다 — 페이로드 커밋의 부모(안내 커밋)가 없다.",
                  "형식 전환 명령(`hint.py branch-transition` · 사람 승인)으로 안내 커밋을 만든다.")
    _ensure_local_commit(repo, remote, tip)
    fmt, why = guide_info(repo, tip)
    if fmt != BRANCH_FORMAT:
        _format_mismatch(f"원격 {HINT_BRANCH_REF} tip", tip, fmt, why)
    return tip, fmt


# ── 형식 전환 (branch-transition · 사람 승인 · 브랜치 push 의 유일한 경로) ───────────────────────────
# 안내 커밋 = README.md 한 개. README 바이트는 추적 템플릿(HEAD)의 바이트 그대로이고 그 첫 줄이 형식 마커여야 한다 — 마커를
# 여기서 덧붙이지 않는다(템플릿 = 배포 바이트 · 한 자리). 템플릿 문안은 D(template) 소유.
_BRANCH_TRANSITION_MESSAGE = f"hint: branch transition → {BRANCH_FORMAT}\n"
_BRANCH_TEMPLATE_REL = f"{core.REL_SKILL}/templates/hint-branch-README.md"


def _transition_template_bytes(repo: Path) -> bytes:
    """추적 HEAD 템플릿을 바이트 그대로 읽는다 · 첫 줄 = 이 발행기의 형식 마커 · 배포면 PII 거부."""
    data = core.git_bytes(repo, "show", f"HEAD:{_BRANCH_TEMPLATE_REL}", check=False)
    if data is None:
        core.fail("HINT_BRANCH_TEMPLATE_ABSENT", f"추적 전환 템플릿이 없다: {_BRANCH_TEMPLATE_REL}")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        core.fail("HINT_BRANCH_TEMPLATE_UNREADABLE", f"전환 템플릿이 UTF-8이 아니다: {_BRANCH_TEMPLATE_REL}")
    if readme_format(text) != BRANCH_FORMAT:
        core.fail("HINT_BRANCH_TEMPLATE_FORMAT",
                  f"전환 템플릿 첫 줄이 형식 마커 `{guide_marker()}` 가 아니다(읽힌 형식 {readme_format(text)!r}).",
                  f"`{_BRANCH_TEMPLATE_REL}` 첫 줄에 `{guide_marker()}` 를 두고 커밋한다(마커를 도구가 덧붙이지 않는다).")
    hits = pii.scan_text(text, pii.require_terms(repo), profile="deploy")
    if hits:
        core.fail("HINT_BRANCH_TEMPLATE_PII", "전환 README 배포 PII: " + pii.render_hits(hits))
    return data


def _transition_tree(repo: Path, readme: bytes, *, write: bool = True) -> str:
    """README.md 한 개(mode 100644) tree SHA. `write=False`는 판정용이라 object DB 쓰기 0."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    try:
        blob_args = ["git", "hash-object", *( ["-w"] if write else []), "--no-filters", "--stdin"]
        out = subprocess.run(blob_args, input=readme, capture_output=True, cwd=str(repo), env=env)
    except OSError as e:
        core.fail("HINT_GIT_UNAVAILABLE", f"git 실행 불가: {e}")
    if out.returncode != 0:
        core.fail("HINT_GIT_FAILED", "전환 README blob hash 실패: " + out.stderr.decode("utf-8", "replace").strip())
    sha = out.stdout.decode("ascii", "replace").strip()
    if write:
        tree = _mktree(repo, [("100644", "blob", sha, GUIDE_README)])
        if tree_entries(repo, tree) != [{"mode": "100644", "type": "blob", "sha": sha, "path": GUIDE_README}]:
            core.fail("HINT_BRANCH_TRANSITION_TREE_MISMATCH", "전환 트리가 README.md 하나(mode 100644)가 아니다.")
        return tree
    try:
        tree_data = b"100644 README.md\0" + bytes.fromhex(sha)
        tr = subprocess.run(["git", "hash-object", "-t", "tree", "--stdin"], input=tree_data,
                            capture_output=True, cwd=str(repo), env=env)
    except (OSError, ValueError) as e:
        core.fail("HINT_GIT_UNAVAILABLE", f"전환 tree SHA 계산 실패: {e}")
    if tr.returncode != 0:
        core.fail("HINT_GIT_FAILED", "전환 tree SHA 계산 실패: " + tr.stderr.decode("utf-8", "replace").strip())
    return tr.stdout.decode("ascii", "replace").strip()


def _transition_commit(repo: Path, parent: str, tree: str, utc: str) -> str:
    """결정론·합성 신원·단일 부모 전환 커밋을 **오브젝트로만** 만든다(ref 이동 ✗). 같은 부모·트리·시각 = 같은 SHA."""
    commit = core.git(repo, "-c", "i18n.commitEncoding=UTF-8", "commit-tree", tree, "--no-gpg-sign", "-p", parent,
                      input_text=_BRANCH_TRANSITION_MESSAGE, env_extra=identity_env(utc)).stdout.strip()
    raw = read_object_text(repo, "commit", commit) or ""
    ids = parse_ident_headers(raw)
    epoch = int(core.parse_utc(utc).timestamp())
    if (not raw.startswith(f"tree {tree}\n") or _commit_parents(raw) != [parent]
            or raw.partition("\n\n")[2] != _BRANCH_TRANSITION_MESSAGE
            or any(ids.get(role, {}).get("ident") != synthetic_identity()
                   or ids.get(role, {}).get("epoch") != epoch for role in ("author", "committer"))):
        core.fail("HINT_BRANCH_TRANSITION_RESULT_MISMATCH", "전환 커밋이 요청한 트리·단일 parent·합성 신원·주입 시각과 다르다.")
    return commit


def _remote_hint_tip(repo: Path, remote: str) -> str | None:
    # Runtime import avoids the tag -> branch module cycle.
    from . import tag  # noqa: PLC0415
    return tag.remote_ref_object(repo, remote, HINT_BRANCH_REF)


def _existing_transition(repo: Path, commit: str, parent: str | None, expected_tree: str) -> bool:
    """이미 있는 commit 이 이 README 전환인지 구조로 판정한다(재시도 UTC 와 무관 · object write 0). parent=None 이면 부모는 묻지
    않는다(단일 부모이기만 하면 된다)."""
    raw = read_object_text(repo, "commit", commit) or ""
    if not raw.startswith(f"tree {expected_tree}\n") or raw.partition("\n\n")[2] != _BRANCH_TRANSITION_MESSAGE:
        return False
    parents = _commit_parents(raw)
    ids = parse_ident_headers(raw)
    return (len(parents) == 1 and (parent is None or parents == [parent])
            and all(ids.get(role, {}).get("ident") == synthetic_identity() for role in ("author", "committer")))


def _orphan_payloads(repo: Path, local: str, keep: str) -> list[str]:
    """로컬 hint 를 옮기면 **태그도 원격 tip 도 붙잡지 않게 되는** 페이로드 커밋(PAYLOAD.json 을 가진 커밋) 목록.
    옛 체인형 로컬 hint(`5408271` 등 · 페이로드 4커밋)는 태그가 자기 커밋과 조상을 붙잡으므로 보통 비어 있다(plan §8 R6)."""
    r = core.git(repo, "rev-list", local, "--not", keep, "--glob=refs/tags/hint/*", check=False)
    if r.returncode != 0:
        core.fail("HINT_GIT_FAILED", f"rev-list 실패(rc={r.returncode}): {r.stderr.strip()}")
    return [c for c in r.stdout.split() if read_blob(repo, c, PAYLOAD_JSON) is not None]


def _require_approval(approved_by, approved_utc) -> str:
    if not isinstance(approved_by, str) or not approved_by.strip() or approved_utc is None:
        core.fail("HINT_APPROVAL_ABSENT",
                  "형식 전환은 원격 브랜치를 바꾼다 — 사람 승인 전사가 필요하다(--approved-by · --approved-utc · plan_26092908 §6 G2). "
                  "어느 ref 도 쓰지 않았다.",
                  "사람의 승인 발화를 전사해 --approved-by \"…\" --approved-utc <UTC> 로 다시 실행한다.")
    return core.require_utc(approved_utc)


def branch_transition(repo: Path, *, remote: str, generated_utc: str, approved_by: str | None = None,
                      approved_utc: str | None = None) -> dict:
    """형식 전환(브랜치 push 의 **유일한** 경로 · 사람 승인 필수): 원격 hint tip 위에 이 발행기 형식의 안내 커밋을 얹고
    `refs/heads/hint:refs/heads/hint` 하나를 non-force 로 민다.

    상태 판정(구조로 · 재시도 UTC 무관):
      - 원격 tip 이 이미 이 템플릿의 BRANCH_FORMAT 안내 → `already-transitioned`(로컬 hint 가 다르면 원격 tip 으로 맞춘다 =
        `local-synced` · 옛 체인형 로컬 hint 정리 · plan §4.7 로컬 정리)
      - 원격 tip 이 더 **새** 형식의 안내 → HINT_BRANCH_TRANSITION_DOWNGRADE(되돌리지 않는다)
      - 로컬 hint 가 원격 tip 을 유일한 부모로 가진 이 전환 커밋(= 첫 push 가 끊김) → 재 push = `push-resumed`
      - 그 밖 → 새 전환 커밋(부모 = 원격 tip · 원격 tip 오브젝트가 없으면 그 한 ref fetch) → 로컬 hint CAS → push = `transitioned`
    로컬 hint 를 옮기기 전: 옮기면 태그도 원격도 붙잡지 않게 되는 페이로드 커밋이 있으면 HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD
    (아무것도 쓰지 않는다). 태그·카탈로그·코드 워크트리는 건드리지 않는다. 승인 전사는 반환값에만 싣는다(배포 오브젝트 ✗ · PII)."""
    utc = core.require_utc(generated_utc)
    if not isinstance(remote, str) or not remote.strip() or remote.lstrip().startswith("-"):
        core.fail("HINT_BRANCH_REMOTE_INVALID", f"전환 remote 인자가 올바르지 않다: {remote!r}")
    appr_utc = _require_approval(approved_by, approved_utc)
    _require_hint_not_checked_out(repo)
    readme = _transition_template_bytes(repo)
    expected_tree = _transition_tree(repo, readme, write=False)
    refspec = f"{HINT_BRANCH_REF}:{HINT_BRANCH_REF}"
    remote_tip = _remote_hint_tip(repo, remote)
    if not remote_tip:
        core.fail("HINT_BRANCH_TRANSITION_REMOTE_ABSENT", f"원격 {remote!r}에 {HINT_BRANCH_REF} tip이 없다 — 첫 브랜치 생성은 "
                                                          "이 명령의 범위 밖이다(사람이 원격을 준비한다).")
    _ensure_local_commit(repo, remote, remote_tip)
    local = hint_tip(repo)
    base = {"remote": remote, "refspec": refspec, "format": BRANCH_FORMAT, "approved_by": approved_by.strip(),
            "approved_utc": appr_utc, "local_before": local, "remote_before": remote_tip}
    remote_fmt, _why = guide_info(repo, remote_tip)
    if _format_num(remote_fmt) > _format_num(BRANCH_FORMAT):
        core.fail("HINT_BRANCH_TRANSITION_DOWNGRADE",
                  f"원격 안내 {remote_tip[:12]} 의 형식 {remote_fmt} 가 이 발행기 {BRANCH_FORMAT} 보다 새것이다 — 되돌리지 않는다.",
                  "발행기(저장소)를 갱신한다.")
    if remote_fmt == BRANCH_FORMAT and _existing_transition(repo, remote_tip, None, expected_tree):
        if local == remote_tip:
            return {**base, "status": "already-transitioned", "new": remote_tip, "local_after": local,
                    "remote_after": remote_tip}
        orphans = _orphan_payloads(repo, local, remote_tip) if local else []
        if orphans:
            core.fail("HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD",
                      f"로컬 {HINT_BRANCH_REF} 를 원격 안내로 맞추면 태그가 붙잡지 않는 페이로드 커밋 {len(orphans)}개가 고아가 된다: "
                      f"{[c[:12] for c in orphans[:10]]} — 아무것도 쓰지 않았다.",
                      "그 커밋을 가리키는 태그가 있어야 하는지 확인한다(태그 없는 페이로드 커밋 = 발행되지 않은 것).")
        _update_hint_ref(repo, remote_tip, local)
        return {**base, "status": "local-synced", "new": remote_tip, "local_after": remote_tip, "remote_after": remote_tip}
    from . import tag  # noqa: PLC0415
    if local and _existing_transition(repo, local, remote_tip, expected_tree):
        tag.git_push_authenticated(repo, remote, refspec, dry_run=False)
        after = _remote_hint_tip(repo, remote)
        if after != local:
            core.fail("HINT_BRANCH_TRANSITION_REMOTE_SHA_MISMATCH",
                      f"재개 push 뒤 원격 {HINT_BRANCH_REF}={str(after)[:12]} ≠ local new {local[:12]}")
        return {**base, "status": "push-resumed", "old": remote_tip, "new": local, "local_after": local,
                "remote_after": after}
    if local:
        # 이전 시도의 전환 후보(README 한 장 · PAYLOAD 없음)는 고아가 되어도 잃는 것이 없다 — 페이로드 커밋만 센다.
        orphans = _orphan_payloads(repo, local, remote_tip)
        if orphans:
            core.fail("HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD",
                      f"로컬 {HINT_BRANCH_REF} 를 옮기면 태그가 붙잡지 않는 페이로드 커밋 {len(orphans)}개가 고아가 된다: "
                      f"{[c[:12] for c in orphans[:10]]} — 아무것도 쓰지 않았다.",
                      "그 커밋을 가리키는 태그가 있어야 하는지 확인한다(태그 없는 페이로드 커밋 = 발행되지 않은 것).")
    tree = _transition_tree(repo, readme, write=True)
    new = _transition_commit(repo, remote_tip, tree, utc)
    _update_hint_ref(repo, new, local)
    tag.git_push_authenticated(repo, remote, refspec, dry_run=False)
    after = _remote_hint_tip(repo, remote)
    if after != new:
        core.fail("HINT_BRANCH_TRANSITION_REMOTE_SHA_MISMATCH", f"push 뒤 원격 {HINT_BRANCH_REF}={str(after)[:12]} ≠ local new {new[:12]}")
    return {**base, "status": "transitioned", "old": remote_tip, "new": new, "local_after": new, "remote_after": after}


def commit_payload(repo: Path, payload_dir, *, message: str, generated_utc: str, guide: str | None = None,
                   remote: str | None = None) -> str:
    """페이로드 → allowlist 트리 → 합성 신원·주입 시각 커밋(부모 = **안내 커밋**) → 반환 = 커밋 SHA. **ref 는 쓰지 않는다**.

    부모(plan_26092908 §4.7): `guide`(전체 SHA · 호출자가 `guide_commit` 으로 한 번 해소해 draft state 에 적은 값) 또는
    `remote`(여기서 `guide_commit(repo, remote)` 로 해소) 중 하나가 필수다 — 둘 다 없으면 HINT_BRANCH_GUIDE_REQUIRED(옛 "로컬
    tip 위에 얹기" 로 조용히 돌아가지 않는다). 둘 다 주면 같은 SHA 여야 한다. 안내는 BRANCH_FORMAT 형식이어야 한다.

    순서가 곧 계약이다 — 모든 거부는 object DB 밖 부작용 0 에서 난다:
      ① 파일 전수(allowlist · 심링크 · 필수)  ② PAYLOAD/PROVENANCE/메시지/이름 재조립/PROMPT 잔재 정합(ⓐ↔ⓒ)
      ③ PII: 페이로드 트리(배포 4종+리터럴) · 경로 이름 · 커밋 메시지 · 커밋 신원  ④ 안내 커밋 판정
      ⑤ 트리 짓기 + 결과 대조(C4)  ⑥ commit-tree(서명 ✗ · 신원·날짜 env · 부모 = 안내) → 결과 대조
    결정론: 같은 페이로드·메시지·시각·안내 = 같은 SHA(재실행 = 같은 오브젝트 · 재개 멱등). 페이로드 커밋은 브랜치에 얹지 않고
    태그만 가리킨다 — 로컬 `refs/heads/hint` · 메인 워킹트리·인덱스·HEAD 는 읽지도 쓰지도 않는다.
    """
    utc = core.require_utc(generated_utc)
    if guide is None and remote is None:
        core.fail("HINT_BRANCH_GUIDE_REQUIRED",
                  "페이로드 커밋의 부모(안내 커밋)가 주어지지 않았다 — guide=<SHA> 또는 remote=<원격> 이 필요하다.",
                  "branch.guide_commit(repo, remote) 로 원격 hint tip(안내 커밋)을 읽어 넘긴다(plan_26092908 §4.7).")
    root = Path(payload_dir).resolve()
    rels = payload_files(root)
    tag = _validate_payload_docs(root, rels, message)
    terms = pii.require_terms(repo)
    hits = pii.scan_tree(root, terms, profile="deploy", rels=rels, shell_default_exempt=True)
    if hits:
        core.fail("HINT_PAYLOAD_PII", f"페이로드 PII {len(hits)}건(배포면 = 4종 전부 fail-closed):\n  "
                  + pii.render_hits(hits), "발췌·산출물을 pii.substitute 치환표로 다시 만든다.")
    # 파일 **이름**도 트리 오브젝트에 실려 배포된다(내용 스캔은 이름을 보지 않는다 — 호스트명이 박힌 env 파일명 등).
    ph = [_fmt_hit("path", h) for h in pii.scan_text("\n".join(rels), terms, profile="deploy")]
    if ph:
        core.fail("HINT_PAYLOAD_PATH_PII", "페이로드 경로 이름 PII: " + "; ".join(ph),
                  "산출물 파일 이름에서 호스트·주소·운영자 경로를 걷어낸다(artifacts 수집기가 이름을 짓는다).")
    mh = [_fmt_hit("message", h) for h in pii.scan_text(message, terms, profile="deploy")]
    if mh:
        core.fail("HINT_COMMIT_MESSAGE_PII", "커밋 메시지 PII: " + "; ".join(mh))
    ih = identity_pii_hits(terms)
    if ih:
        core.fail("HINT_COMMIT_IDENTITY_PII", "합성 신원이 PII 에 걸린다: " + "; ".join(ih))

    if guide is not None:
        parent = require_guide(repo, guide, where="지정 안내 커밋")
        if remote is not None:
            live, _fmt = guide_commit(repo, remote)
            if live != parent:
                core.fail("HINT_BRANCH_GUIDE_MOVED",
                          f"지정 안내 {parent[:12]} ≠ 원격 {remote!r} 의 현재 안내 {live[:12]} — 그 사이 형식 전환이 있었다.",
                          "새 안내로 다시 발행한다(draft state 의 안내를 갱신).")
    else:
        parent, _fmt = guide_commit(repo, remote)
    tree = build_tree(repo, root, rels)
    want_epoch = int(core.parse_utc(utc).timestamp())
    # `--no-gpg-sign`: commit-tree 도 commit.gpgSign 설정을 따른다 — 운영자 서명이 배포 오브젝트에 실리면 SHA 가 운영자
    # 키의 함수가 된다. `i18n.commitEncoding=UTF-8`: 운영자가 다른 인코딩을 설정해 두면 `encoding` 헤더가 붙어 같은
    # 입력의 SHA 가 설정 따라 달라진다(결정론 = 같은 입력 → 같은 SHA · K2).
    commit = core.git(repo, "-c", "i18n.commitEncoding=UTF-8", "commit-tree", tree, "--no-gpg-sign", "-p", parent,
                      input_text=message, env_extra=identity_env(utc)).stdout.strip()
    raw = read_object_text(repo, "commit", commit) or ""
    ids = parse_ident_headers(raw)
    if (not raw.startswith(f"tree {tree}\n") or _commit_parents(raw) != [parent]
            or raw.partition("\n\n")[2] != message
            or any(ids.get(k, {}).get("ident") != synthetic_identity()
                   or ids.get(k, {}).get("epoch") != want_epoch for k in ("author", "committer"))):
        core.fail("HINT_COMMIT_RESULT_MISMATCH", f"지은 커밋 {commit[:12]} 이 요청(트리·부모=안내·메시지·합성 신원·주입 시각)과 다르다.")
    _log(f"페이로드 커밋 {commit[:12]} ({tag} · {len(rels)} 파일 · 부모 = 안내 {parent[:12]} · 브랜치 불이동 — 태그만 가리킨다)")
    return commit


# ── 읽기 · 앵커 게이트 ─────────────────────────────────────────────────────────────────────────
def read_blob(repo: Path, rev: str, path: str) -> bytes | None:
    """`<rev>:<path>` blob 바이트. 없거나 blob 이 아니면 None(배관 읽기 · 워킹트리 무관)."""
    return core.git_bytes(repo, "cat-file", "blob", f"{rev}:{path}", check=False)


def read_json_blob(repo: Path, rev: str, path: str) -> tuple[dict | None, str | None]:
    """(문서, 오류). 부재 = (None, "absent") · 깨짐 = (None, "unreadable: …")."""
    raw = read_blob(repo, rev, path)
    if raw is None:
        return None, "absent"
    try:
        doc = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as e:
        return None, f"unreadable: {e!r}"
    if not isinstance(doc, dict):
        return None, "unreadable: JSON 객체가 아니다"
    return doc, None


def require_payload_anchor(repo: Path, tag: str, anchor: str) -> None:
    """계약 §6 집행 — **태그는 페이로드 커밋을 가리키고, 그 커밋의 부모는 안내 커밋이다**(plan_26092908 §4.7).

    ★ 2026-09-07 신설(plan_26090715 §5 ①-a · audit_26090708 §2). 이 검사가 없어서 발행자가 조립·브랜치 커밋을
      통째로 건너뛰고 **소스 트리 커밋**에 봉인해도 finalize/seal/verify 가 전부 통과했다(fail-open). native 3종이
      그렇게 나갔고, single 태그가 multi-node 커밋에 · multi 태그가 single-node 커밋에 앵커되는 형태까지 갔다 —
      배포 zip 에 산출물 대신 저장소 소스가 담겼다.
    ★ 2026-09-29 V12: 옛 A("앵커가 로컬 hint tip 의 조상인가")는 PC 마다 갈라진 체인에서 다른 PC 태그를 막았고, 로컬 브랜치
      상태에 판정이 묶였다. 이제 A 는 **앵커 커밋 자신**만 본다 — 로컬 ref·네트워크 무관(태그를 fetch 하면 부모도 딸려 온다).
    세 가지를 저장소 안 git 객체만으로 묻는다(네트워크 ✗ · 해시 재기재 ✗):
      A. 앵커의 부모가 정확히 하나이고 그것이 형식 마커를 가진 안내 커밋인가 → HINT_ANCHOR_PARENT_NOT_GUIDE
         (형식은 묻지 않는다 — 뒤 형식 전환 뒤에도 앞 형식 안내 위의 태그는 유효하다 · 마커 도입 전 체인형 태그는 판정 대상이
         아니다 · D10 · P1)
      B. 앵커 트리에 PAYLOAD.json 이 있는가       → HINT_ANCHOR_PAYLOAD_ABSENT
      C. 앵커 트리 PROVENANCE.tag == 이 태그인가   → HINT_ANCHOR_PROVENANCE_TAG_MISMATCH(남의 페이로드 재사용 차단)
    """
    commit = ""
    if anchor:
        commit = core.git(repo, "rev-parse", "--verify", "--quiet", f"{anchor}^{{commit}}", check=False).stdout.strip()
    if not commit:
        core.fail("HINT_ANCHOR_UNRESOLVED", f"앵커를 커밋으로 해소하지 못했다: {anchor!r}")
    parents = _commit_parents(read_object_text(repo, "commit", commit) or "")
    why = None
    if len(parents) != 1:
        why = f"부모가 {len(parents)}개다(페이로드 커밋의 부모는 안내 커밋 하나)"
    else:
        _fmt, why = guide_info(repo, parents[0])
    if why:
        core.fail("HINT_ANCHOR_PARENT_NOT_GUIDE",
                  f"앵커 {commit[:12]} 의 부모가 안내 커밋이 아니다 — {why}. 계약 §6: 태그는 안내 커밋 위의 페이로드 커밋을 "
                  "가리킨다. 소스 트리 커밋에 봉인하면 zip 에 산출물 대신 저장소 소스가 담긴다(2026-09-07 실증: native 3종).",
                  "hint.py continue 로 페이로드 커밋부터 만든다(부모 = 원격 hint tip 의 안내 커밋 · 봉인은 그 커밋에만 한다).")
    if read_blob(repo, commit, PAYLOAD_JSON) is None:
        core.fail("HINT_ANCHOR_PAYLOAD_ABSENT", f"앵커 {commit[:12]} 트리에 PAYLOAD.json 이 없다 — 페이로드 커밋이 아니다.")
    prov, err = read_json_blob(repo, commit, PROVENANCE_JSON)
    if err == "absent":
        core.fail("HINT_ANCHOR_PROVENANCE_ABSENT",
                  f"앵커 {commit[:12]} 트리에 PROVENANCE.json 이 없다 — 어느 태그를 위한 페이로드인지 말하지 않는다.")
    if prov is None:
        core.fail("HINT_ANCHOR_PROVENANCE_UNREADABLE", f"앵커 {commit[:12]} PROVENANCE.json {err}")
    if prov.get("tag") != tag:
        core.fail("HINT_ANCHOR_PROVENANCE_TAG_MISMATCH",
                  f"앵커의 PROVENANCE.tag={prov.get('tag')!r} 인데 봉인하려는 태그는 {tag!r} 이다 — 남의 페이로드다.")


def read_blobs(repo: Path, shas) -> dict[str, bytes]:
    """blob SHA 여럿 → {sha: bytes}. `cat-file --batch` 한 번(트리 전수 PII 스캔용 · 파일마다 프로세스를 띄우지 않는다)."""
    uniq = sorted({s for s in shas})
    if not uniq:
        return {}
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    r = subprocess.run(["git", "cat-file", "--batch"], input=("\n".join(uniq) + "\n").encode("ascii"),
                       capture_output=True, cwd=str(repo), env=env)
    if r.returncode != 0:
        core.fail("HINT_GIT_FAILED", f"git cat-file --batch 실패(rc={r.returncode}): "
                                    f"{r.stderr.decode('utf-8', 'replace').strip()}")
    data, pos, out = r.stdout, 0, {}
    for want in uniq:
        nl = data.find(b"\n", pos)
        head = data[pos:nl].decode("ascii", "replace").split() if nl >= 0 else []
        if len(head) != 3 or head[0] != want or head[1] != "blob" or not head[2].isdigit():
            core.fail("HINT_TREE_READ_FAILED", f"blob 을 읽을 수 없다: {want} → {' '.join(head) or '출력 없음'}")
        size = int(head[2])
        out[want] = data[nl + 1:nl + 1 + size]
        pos = nl + 1 + size + 1
    return out


def export_tree(repo: Path, rev: str, dest) -> list[str]:
    """`<rev>` 트리를 **빈** 디렉터리 `dest` 에 파일로 풀어 쓴다(배관 읽기 · 체크아웃 ✗ · 2026-09-22 통합). 반환 = 쓴 상대경로(정렬).

    쓰임: hint.py 의 서사 린터 주입구(`tag.verify_local(lint_fn=…)`) — 린터는 디렉터리를 읽으므로, 봉인된 **git 오브젝트**의
    바이트(디스크의 draft 가 아니라)를 임시 디렉터리에 옮겨 검사한다(배포되는 것은 오브젝트다 · commit_violations 와 같은 이유).
    심링크(120000)·서브모듈(160000)은 페이로드 계약 밖이라 쓰지 않고 실패한다 · 경로는 allowlist 판정을 다시 통과해야 한다
    (`../` 같은 이름이 dest 밖으로 쓰지 못하게). 실행비트(100755)는 보존한다."""
    root = Path(dest)
    if root.exists() and (not root.is_dir() or any(root.iterdir())):
        core.fail("HINT_EXPORT_DEST_NOT_EMPTY", f"풀어 쓸 자리가 비어 있지 않다: {root}")
    entries = tree_entries(repo, rev)
    bad = [e["path"] for e in entries if e["type"] != "blob" or e["mode"] not in ("100644", "100755")]
    if bad:
        core.fail("HINT_EXPORT_ENTRY_UNSUPPORTED", f"블롭(100644·100755)이 아닌 항목: {bad[:10]}")
    outside = [f"{e['path']} ({why})" for e in entries if (why := allowlist_violation(e["path"]))]
    if outside:
        core.fail("HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST", f"allowlist 밖 경로를 풀어 쓰지 않는다: {outside[:10]}")
    blobs = read_blobs(repo, [e["sha"] for e in entries])
    root.mkdir(parents=True, exist_ok=True)
    for e in entries:
        out = root / e["path"]
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(blobs[e["sha"]])
        if e["mode"] == "100755":
            out.chmod(0o755)
    return sorted(e["path"] for e in entries)


def commit_violations(repo: Path, commit: str, tag: str, terms: list[str]) -> list[str]:
    """이미 있는 페이로드 커밋 하나가 배포 계약을 지키는가(봉인 전·후 · `tag.seal`/`tag.verify_local` 이 부른다 · D10:
    **이 태그의 커밋만**). `CODE: 설명` 정렬 목록(빈 = 계약 준수). 읽기만 한다.

    커밋 전 디스크 검사(`commit_payload`)와 같은 계약을 **git 이 실제로 든 바이트**에서 다시 묻는다 — 커밋이 이 도구가
    아닌 경로(손 배관 · 다른 PC · 옛 도구)로 만들어졌을 수 있고, 배포되는 것은 디스크가 아니라 오브젝트다:
      - 트리 allowlist(`tree_violations`)
      - author/committer = 합성 신원(★K1: 57/57 페이로드 커밋이 운영자 신원이었다 — PII 목록 스캔으로는 목록에 없는
        실제 주소를 못 막으므로 **동일성**으로 판정한다) + 신원 PII tripwire
      - 커밋 메시지 PII(배포 메타데이터다)
      - PAYLOAD/PROVENANCE/메시지/이름 재조립/PROMPT 잔재 — `payload_doc_problems`(디스크와 같은 함수)
      - blob 전수 PII(배포 4종+리터럴 · compose 셸 기본값 면제는 커밋 전과 같다) + 경로 이름 PII
    """
    probs: list[str] = []
    raw = read_object_text(repo, "commit", commit)
    if raw is None:
        return [f"HINT_COMMIT_UNREADABLE: 커밋 오브젝트를 읽을 수 없다: {commit}"]
    probs.extend(tree_violations(repo, commit))
    ids = parse_ident_headers(raw)
    for role in ("author", "committer"):
        who = ids.get(role, {}).get("ident")
        if who != synthetic_identity():
            probs.append(f"HINT_COMMIT_IDENTITY_NOT_SYNTHETIC: {role} 가 합성 신원이 아니다"
                         f"({'부재' if who is None else '다른 신원'}) — 운영자 신원이 zip 수신자에게 배포된다(K1).")
        elif identity_pii_hits(terms, who):
            probs.append(f"HINT_COMMIT_IDENTITY_PII: {role} 신원이 PII 목록에 걸린다")
    message = raw.partition("\n\n")[2]
    mh = pii.scan_text(message, terms, profile="deploy")
    if mh:
        probs.append("HINT_COMMIT_MESSAGE_PII: " + pii.render_hits(mh, limit=5))
    all_entries = tree_entries(repo, commit)
    all_paths = sorted(e["path"] for e in all_entries)
    entries = [e for e in all_entries if e["type"] == "blob" and e["mode"] != "120000"]
    blobs = read_blobs(repo, [e["sha"] for e in entries])
    by_path = {e["path"]: blobs[e["sha"]] for e in entries}

    def _json(path: str):
        data = by_path.get(path)
        if data is None:
            return None
        try:
            return json.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            return None

    docs = {n: by_path[n] for n in PAYLOAD_DOCS if n in by_path}
    if PAYLOAD_JSON in by_path and PROVENANCE_JSON in by_path:
        # 읽지 못한 JSON·객체가 아닌 JSON 은 payload_doc_problems 가 UNREADABLE 로 판정한다(여기서 건너뛰지 않는다).
        probs.extend(payload_doc_problems(_json(PAYLOAD_JSON), _json(PROVENANCE_JSON), paths=all_paths,
                                          message=message, docs=docs, expect_tag=tag))
    else:
        probs.extend(doc_residue_problems(docs))   # 필수 부재는 tree_violations 가 보고한다 — 잔재 검사는 그래도 돈다
    # blob 스캔 규칙은 scan_blobs 한 벌(바이너리 = 내용 판정 · compose 셸 기본값 면제 = 커밋 전 scan_tree 와 같다).
    # 면제 기록은 stderr 로 남긴다(조용히 넘기지 않는다 · 2026-09-01).
    hits = scan_blobs(((path, by_path[path]) for path in sorted(by_path)), terms)
    if hits:
        probs.append(f"HINT_PAYLOAD_PII: {len(hits)}건 — " + "; ".join(hits[:10]))
    ph = pii.scan_text("\n".join(all_paths), terms, profile="deploy")
    if ph:
        probs.append("HINT_PAYLOAD_PATH_PII: " + "; ".join(_fmt_hit("path", h) for h in ph[:10]))
    return sorted(set(probs))


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
# 픽스처 문자열은 **조각으로 조립**한다 — 추적 파일에 IPv4·메일·운영자 경로 모양 리터럴을 쓰면 배포 4종 스캔이
# 규칙 문서 자신에 걸린다(2026-08-06 자기스캔 사고).
FIXTURE_TERM = "fixture-pii-term-zeta"
_FX_OPERATOR_NAME = "Operator Realname"
_FX_OPERATOR_EMAIL = "@".join(["operator.realname", "corp.example"])
_FX_UTC = "2026-09-21T10:21:01Z"
# v7 이름(결정론 3축 + 자율 꼬리 · plan_26092908 §4.1). 꼬리는 v6 뒤 3축 모양이 아니다(그 모양은 기발행 v6 로 읽힌다).
_FX_TAG = ("hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/"
           "qnvfp4-len262144-kvauto-mmp-eager")
_SCRUB_ENV = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_COMMON_DIR",
              "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_PREFIX", "GIT_NAMESPACE", "GIT_CONFIG",
              "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS", "GITHUB_TOKEN", "HINT_PUSH_TOKEN",
              "GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE",
              "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL", "GIT_COMMITTER_DATE")


@contextlib.contextmanager
def selftest_env(tmp: Path, extra: dict[str, str] | None = None):
    """자체검사 격리: 운영자 전역·시스템 git 설정(서명·훅·신원)과 토큰 env 를 끊는다. 끝나면 원복한다.
    extra 로 운영자 신원 env(GIT_AUTHOR_* 등)를 **일부러** 심어 합성 신원 강제를 시험할 수 있다."""
    gcfg = tmp / "gitconfig"
    gcfg.write_text("[init]\n\tdefaultBranch = main\n[commit]\n\tgpgSign = false\n[tag]\n\tgpgSign = false\n"
                    f"[core]\n\thooksPath = {tmp / 'no-hooks'}\n", encoding="utf-8")
    keys = set(_SCRUB_ENV) | {"GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM"} | set(extra or {})
    saved = {k: os.environ.get(k) for k in keys}
    try:
        for k in _SCRUB_ENV:
            os.environ.pop(k, None)
        os.environ["GIT_CONFIG_GLOBAL"] = str(gcfg)
        os.environ["GIT_CONFIG_NOSYSTEM"] = "1"
        for k, v in (extra or {}).items():
            os.environ[k] = v
        yield
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def selftest_repo(tmp: Path, name: str = "repo") -> Path:
    """격리 저장소: main 에 추적 파일 1 커밋(운영자 신원 설정) + 비추적 픽스처 pii_terms.
    ★ 실제 `.claude/pii_terms.txt` 는 픽스처로 복사하지 않는다."""
    repo = tmp / name
    repo.mkdir(parents=True)
    core.git(repo, "init", "-q", "-b", "main")
    core.git(repo, "config", "user.name", _FX_OPERATOR_NAME)
    core.git(repo, "config", "user.email", _FX_OPERATOR_EMAIL)
    (repo / "src.txt").write_text("source\n", encoding="utf-8")
    core.git(repo, "add", "src.txt")
    core.git(repo, "commit", "-qm", "source")
    terms = repo / core.REL_PII_TERMS
    terms.parent.mkdir(parents=True, exist_ok=True)
    terms.write_text(f"# fixture\n{FIXTURE_TERM}\n", encoding="utf-8")
    return repo


FX_GUIDE_BODY = "# hint 브랜치 안내(픽스처)\n\n이 브랜치는 hint 태그 페이로드 전용이다.\n"


def selftest_guide(repo: Path, *, fmt: str | None = BRANCH_FORMAT, parent: str | None = None,
                   body: str = FX_GUIDE_BODY, utc: str | None = None) -> str:
    """자체검사용 안내 커밋 **오브젝트**(ref 이동 ✗). fmt=None = 마커 없는 옛 안내(v6 전환 커밋 `6294a91` 모양).
    결정론(같은 인자 = 같은 SHA) — 다른 저장소에서 같은 페이로드 커밋 SHA 를 재현하는 자체검사가 쓴다(tag·hint 공유)."""
    readme = (guide_marker(fmt) + "\n" if fmt else "") + body
    blob = core.git(repo, "hash-object", "-w", "--no-filters", "--stdin", input_text=readme).stdout.strip()
    tree = _mktree(repo, [("100644", "blob", blob, GUIDE_README)])
    args = ["-c", "i18n.commitEncoding=UTF-8", "commit-tree", tree, "--no-gpg-sign"] + (["-p", parent] if parent else [])
    return core.git(repo, *args, input_text="hint: guide (fixture)\n",
                    env_extra=identity_env(utc or _FX_UTC)).stdout.strip()


FX_BRIEF = "픽스처 셀의 요약 문단이다 — 지도이지 정답이 아니다."


def fixture_payload_doc(tag: str) -> dict:
    """자체검사용 PAYLOAD.json — naming 재조립 검사(`naming.compose_from_payload`)를 통과하는 **v7** 모양(tag.selftest 공유 ·
    plan_26092908 §4.1). 축 값은 태그 자신을 naming 파서로 해체해 채운다 — 픽스처가 문법을 손으로 다시 적지 않게(두 자리 →
    갈라짐). 기본 레시피(q·len·kv)의 v7 naming 에 이름의 꼬리·timestamp 를 `naming.apply_tail` 로 얹는다(v6 모양 이름도 꼬리 3토큰으로
    같은 이름이 재조립된다)."""
    t = naming.parse_tag(tag)
    arch = naming.parse_arch(t.arch) or {}
    sp = naming.split_recipe(t.recipe) or {}
    axes = {k: {"value": str(arch.get(k, "")), "source": "fixture:manifest"} for k in naming.ARCH_AXES}
    axes.update({k: {"value": str(sp.get(k, "")), "source": "fixture:lockset"} for k in naming.RECIPE_AXES})
    doc = {"grammar": naming.GRAMMAR_V7,
           "segments": {"vllm": {"value": t.vllm, "source": "fixture:vllm_build_input"},
                        "model": {"value": t.model, "source": "fixture:hf_repo"},
                        "arch": {"value": t.arch, "source": "derived"},
                        "recipe": {"value": sp.get("base", t.recipe), "source": "derived(q·len·kv)"}},
           "axes": axes,
           "vllm_build_input": {"kind": "release", "ref": "v" + t.vllm, "sha": None, "prev_release": None}}
    if t.base_tag is None:
        # 옛 세대 이름(음성대조 픽스처): 재조립할 축이 없다 — 이름 문법 거부(validate_new_name)가 먼저 보고되게 모양만 둔다.
        return {"schema_version": 2, "format": PAYLOAD_FORMAT, "tag": tag, "generated_utc": _FX_UTC, "naming": doc}
    tail = [{"token": tok, "meaning": f"픽스처 꼬리 {tok}", "evidence": None} for tok in sp.get("tail") or []]
    nm = naming.apply_tail(doc, tail, sp.get("timestamp"))
    return {"schema_version": 2, "format": PAYLOAD_FORMAT, "tag": tag, "generated_utc": _FX_UTC, "naming": nm}


def fixture_hint_doc(brief: str = FX_BRIEF) -> str:
    """00-hint.md 픽스처 — §0.1 첫 문단이 annotation brief 의 기계 추출 원천이다(template.brief 규칙)."""
    return f"# hint\n\n## 0.1 요약\n\n{brief}\n\n## 0.2 유효맥락\n\n픽스처 본문.\n"


def write_fixture_payload(root: Path, tag: str, *, source_anchor: str | None,
                          extra_files: dict[str, str] | None = None, payload_doc: dict | None = None,
                          brief: str = FX_BRIEF) -> None:
    """v6 최상위 8종 + 슬롯 산출물(실행비트·4단 중첩·compose 셸 기본값) 픽스처."""
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    shell_default = "${NAS_MODEL_PATH:-" + "/" + "mnt" + "/models}"
    files = {
        "README.md": "# hint payload\n",
        "00-hint.md": fixture_hint_doc(brief),
        "01-artifacts.md": "## 1.1 적용 판정\n",
        "02-narrative.md": "## 2.1 출발점\n",
        "03-benchmark.md": "## 3 측정\n",
        "LINEAGE.json": core.dumps({"schema_version": 1, "documents": []}),
        "artifacts/triplet/model.yaml": "max-model-len: 262144\n",
        "artifacts/compose/docker-compose.yaml": f"volumes:\n  - {shell_default}:/models:ro\n",
        "artifacts/build_recipe/a/b/c/deep.txt": "4단 중첩\n",
    }
    files.update(extra_files or {})
    for rel, text in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
    runner = root / "artifacts/build_patch_post/30-fixture.sh"
    runner.parent.mkdir(parents=True, exist_ok=True)
    runner.write_text("#!/bin/sh\necho patch\n", encoding="utf-8")
    runner.chmod(0o755)
    core.write_json(root / PAYLOAD_JSON, payload_doc if payload_doc is not None else fixture_payload_doc(tag))
    rels = sorted({str(p.relative_to(root).as_posix()) for p in root.rglob("*") if p.is_file()} | {PROVENANCE_JSON})
    core.write_json(root / PROVENANCE_JSON, {"schema_version": 2, "tag": tag, "source_anchor": source_anchor,
                                             "source_anchor_is_head": True, "assembly_branch": "main",
                                             "payload_files": rels, "generated_utc": _FX_UTC})


def code_of(fn) -> str | None:
    """fn() 이 낸 HintError 의 code(성공 = None). 자체검사 음성대조용(tag.selftest 공유)."""
    try:
        fn()
    except core.HintError as e:
        return e.code
    return None


def expect_code(fn, code: str) -> bool:
    return code_of(fn) == code


def selftest() -> list[str]:
    """실패 메시지 목록(빈 목록 = 통과). 격리 임시 저장소만 쓴다 — 라이브 hint 브랜치·태그·캠페인 무관."""
    bad: list[str] = []

    def ck(name: str, cond) -> None:
        if not cond:
            bad.append(f"branch: {name}")

    # ── 순수: allowlist 판정
    ck("allowlist 최상위 허용", allowlist_violation("00-hint.md") is None)
    ck("allowlist 슬롯 허용", allowlist_violation("artifacts/compose/sub_recipe.json") is None)
    ck("allowlist 슬롯 안 숨김파일 허용(env 형상)", allowlist_violation("artifacts/triplet/.env.m") is None)
    ck("★allowlist 밖 최상위 차단", allowlist_violation("notes.txt") is not None)
    ck("★allowlist 미지 슬롯 차단", allowlist_violation("artifacts/unknown/x") is not None)
    ck("★allowlist 슬롯 디렉터리 자체 파일 차단", allowlist_violation("artifacts/triplet") is not None)
    ck("★.claude 차단(A1)", allowlist_violation(".claude/x.md") is not None)
    ck("★중첩 .git 도 차단(옛 FORBIDDEN_TOP 은 최상위만 봤다)",
       allowlist_violation("artifacts/triplet/.git/config") is not None)
    ck("★상위탈출·절대경로 차단", allowlist_violation("../x") is not None and allowlist_violation("/x") is not None)
    ck("★슬롯 안 .gitattributes·.gitmodules 차단(archive 가 export-ignore 로 zip ≠ 검증 트리)",
       allowlist_violation("artifacts/compose/.gitattributes") is not None
       and allowlist_violation("artifacts/triplet/.gitmodules") is not None)
    ck("★UTF-8 로 표현할 수 없는 이름 차단(surrogate)", allowlist_violation("artifacts/triplet/x\udcff.yaml") is not None)
    ck("합성 신원 = core 상수", synthetic_identity() == f"{core.SYNTHETIC_NAME} <{core.SYNTHETIC_EMAIL}>")
    ck("신원 헤더 파서", parse_ident_headers("tree t\nauthor A B <c> 10 +0000\n\nmsg")["author"]["ident"] == "A B <c>")

    # ── artifacts.SLOTS 교차검증(정적 목록 2자리 · workflow.md 차선책)
    try:
        from . import artifacts as _art  # noqa: PLC0415 — 자체검사 안에서만(순환 import 회피)
        ck("PAYLOAD_SLOT_DIRS == artifacts.SLOTS", tuple(_art.SLOTS) == PAYLOAD_SLOT_DIRS)
    except ImportError as e:
        bad.append(f"branch: artifacts.SLOTS 교차검증 불가 — {e!r}")

    sink = io.StringIO()
    with tempfile.TemporaryDirectory(prefix="hint-branch-selftest-") as td, contextlib.redirect_stderr(sink):
        tmp = Path(td)
        operator_env = {"GIT_AUTHOR_NAME": _FX_OPERATOR_NAME, "GIT_AUTHOR_EMAIL": _FX_OPERATOR_EMAIL,
                        "GIT_COMMITTER_NAME": _FX_OPERATOR_NAME, "GIT_COMMITTER_EMAIL": _FX_OPERATOR_EMAIL}
        with selftest_env(tmp):
            try:
                _selftest_git(tmp, ck, operator_env)
                _transition_selftest(tmp, ck)
            except core.HintError as e:
                bad.append(f"branch: 자체검사 중 예기치 않은 HintError {e.code}: {e.message[:300]}")
            except Exception as e:  # noqa: BLE001 — 자체검사는 죽지 않고 실패로 보고한다
                bad.append(f"branch: 자체검사 중 예외 {type(e).__name__}: {str(e)[:300]}")
    return bad


def _selftest_git(tmp: Path, ck, operator_env: dict) -> None:
    repo = selftest_repo(tmp)
    source = core.git_out(repo, "rev-parse", "HEAD")
    pay = tmp / "payload"
    write_fixture_payload(pay, _FX_TAG, source_anchor=source)

    rels = payload_files(pay)
    ck("payload_files = 최상위 8 + 슬롯 4", len(rels) == 12 and rels == sorted(rels))

    # ── 음성대조: allowlist 밖 · 심링크 · 필수 부재
    (pay / "notes.txt").write_text("x\n", encoding="utf-8")
    ck("★allowlist 밖 파일 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST",
       expect_code(lambda: payload_files(pay), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"))
    (pay / "notes.txt").unlink()
    (pay / "artifacts/unknown").mkdir()
    (pay / "artifacts/unknown/x.sh").write_text("x\n", encoding="utf-8")
    ck("★미지 슬롯 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST",
       expect_code(lambda: payload_files(pay), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"))
    (pay / "artifacts/unknown/x.sh").unlink()
    (pay / "artifacts/unknown").rmdir()
    (pay / "artifacts/triplet/link.yaml").symlink_to(repo / "src.txt")
    ck("★심링크 = HINT_PAYLOAD_SYMLINK", expect_code(lambda: payload_files(pay), "HINT_PAYLOAD_SYMLINK"))
    (pay / "artifacts/triplet/link.yaml").unlink()
    lin = (pay / "LINEAGE.json").read_bytes()
    (pay / "LINEAGE.json").unlink()
    ck("★필수 부재 = HINT_PAYLOAD_REQUIRED_ABSENT",
       expect_code(lambda: payload_files(pay), "HINT_PAYLOAD_REQUIRED_ABSENT"))
    (pay / "LINEAGE.json").write_bytes(lin)

    msg = commit_message(pay, generated_utc=_FX_UTC)
    ck("파생 커밋 메시지에 태그·소스 앵커", _FX_TAG in msg and source in msg)

    # ── build_tree 입력 규율(옛 hint_branch 음성대조 이관): 실물 부재 · 중복 · allowlist 밖 — 부재를 성공으로 넘기지 않는다
    ck("★build_tree 실물 부재 = HINT_PAYLOAD_FILE_ABSENT",
       expect_code(lambda: build_tree(repo, pay, rels + ["artifacts/triplet/nope.yaml"]), "HINT_PAYLOAD_FILE_ABSENT"))
    ck("★build_tree 중복 경로 = HINT_TREE_BUILD_FAILED",
       expect_code(lambda: build_tree(repo, pay, rels + [rels[0]]), "HINT_TREE_BUILD_FAILED"))
    ck("★build_tree allowlist 밖 = HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST",
       expect_code(lambda: build_tree(repo, pay, rels + ["../src.txt"]), "HINT_PAYLOAD_TREE_OUTSIDE_ALLOWLIST"))

    # ── 안내 커밋(plan_26092908 §4.7): 부모 필수 · 형식 판정
    guide = selftest_guide(repo)
    old_guide = selftest_guide(repo, fmt=None)                  # 마커 없는 옛 안내(6294a91 모양)
    v6_guide = selftest_guide(repo, fmt="v6")                   # 마커는 있으나 다른 형식
    ck("readme_format: 마커 첫 줄 = v7 · 마커 없음 = None · 둘째 줄 마커는 무효",
       readme_format(guide_marker() + "\n# x\n") == "v7" and readme_format("# x\n") is None
       and readme_format("# x\n" + guide_marker() + "\n") is None)
    ck("guide_info(v7 안내) = (v7, None)", guide_info(repo, guide) == ("v7", None))
    ck("★마커 없는 옛 안내 = 안내 아님(사유에 '마커')", guide_info(repo, old_guide)[0] is None
       and "마커" in (guide_info(repo, old_guide)[1] or ""))
    ck("★소스 커밋(트리가 README 하나가 아님) = 안내 아님", guide_info(repo, source)[0] is None)
    ck("★require_guide: 마커 없는 옛 안내 = HINT_BRANCH_FORMAT_MISMATCH",
       expect_code(lambda: require_guide(repo, old_guide), "HINT_BRANCH_FORMAT_MISMATCH"))
    ck("★require_guide: 다른 형식(v6 마커) = HINT_BRANCH_FORMAT_MISMATCH",
       expect_code(lambda: require_guide(repo, v6_guide), "HINT_BRANCH_FORMAT_MISMATCH"))
    ck("guide_commit(pinned) = (sha, v7) · 네트워크 0", guide_commit(repo, "unused", pinned=guide) == (guide, "v7"))
    ck("★부모 미지정 = HINT_BRANCH_GUIDE_REQUIRED",
       expect_code(lambda: commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC), "HINT_BRANCH_GUIDE_REQUIRED"))
    ck("★마커 없는 옛 안내 위에는 얹지 않는다 = HINT_BRANCH_FORMAT_MISMATCH",
       expect_code(lambda: commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC, guide=old_guide),
                   "HINT_BRANCH_FORMAT_MISMATCH"))
    ck("★소스 커밋을 안내로 = HINT_BRANCH_FORMAT_MISMATCH",
       expect_code(lambda: commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC, guide=source),
                   "HINT_BRANCH_FORMAT_MISMATCH"))
    ck("거부들 뒤 로컬 hint 부재 그대로", hint_tip(repo) is None)

    # ── 커밋: 메인 워킹트리·인덱스·HEAD 불변 + 합성 신원 + 주입 시각(운영자 env 를 일부러 심는다)
    def snap():
        idx = repo / ".git" / "index"
        return (core.git_out(repo, "status", "--porcelain"), core.git_out(repo, "rev-parse", "HEAD"),
                hashlib.sha256(idx.read_bytes()).hexdigest(), core.git_out(repo, "symbolic-ref", "HEAD"))
    before = snap()
    with selftest_env(tmp, operator_env):
        c1 = commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC, guide=guide)
    ck("메인 status·HEAD·인덱스·체크아웃 불변", snap() == before)
    ck("★로컬 hint 는 움직이지 않는다(부재 그대로 · 페이로드 커밋은 태그만 가리킨다)", hint_tip(repo) is None)
    ck("★부모 = 안내 커밋(하나)", _commit_parents(read_object_text(repo, "commit", c1) or "") == [guide])
    ids = commit_identity(repo, c1)
    ck("★author/committer = 합성 신원(운영자 config·env 무시)",
       ids.get("author", {}).get("ident") == synthetic_identity()
       and ids.get("committer", {}).get("ident") == synthetic_identity())
    raw1 = core.git_bytes(repo, "cat-file", "commit", c1)
    ck("★운영자 신원이 커밋 오브젝트에 없다",
       _FX_OPERATOR_NAME.encode() not in raw1 and _FX_OPERATOR_EMAIL.encode() not in raw1)
    ck("커밋 시각 = 주입 시각", ids.get("author", {}).get("epoch") == int(core.parse_utc(_FX_UTC).timestamp()))
    ck("트리 = payload_files", tree_paths(repo, c1) == rels)
    modes = {e["path"]: e["mode"] for e in tree_entries(repo, c1)}
    ck("실행비트 보존 100755", modes.get("artifacts/build_patch_post/30-fixture.sh") == "100755")
    ck("일반파일 100644", modes.get("00-hint.md") == "100644")
    ck("★회귀 4단 중첩 보존", "artifacts/build_recipe/a/b/c/deep.txt" in modes)
    exp = tmp / "export"
    got = export_tree(repo, c1, exp)
    ck("export_tree = 트리 전수 · 바이트 동일 · 실행비트", got == rels and all(
        (exp / r).read_bytes() == (pay / r).read_bytes() for r in rels)
       and os.access(exp / "artifacts/build_patch_post/30-fixture.sh", os.X_OK))
    ck("★export_tree 비어 있지 않은 자리 = 거부", expect_code(lambda: export_tree(repo, c1, exp), "HINT_EXPORT_DEST_NOT_EMPTY"))
    ck("compose 셸 기본값은 커밋을 막지 않았다", "artifacts/compose/docker-compose.yaml" in modes)
    ck("tree_violations 0", tree_violations(repo, c1) == [])
    ck("read_blob 실재", read_blob(repo, c1, "README.md") == b"# hint payload\n")
    ck("read_blob 부재 = None", read_blob(repo, c1, "nope.md") is None)

    # ── 결정론: 별도 저장소 · 같은 입력 = 같은 SHA
    repo2 = tmp / "repo2"
    repo2.mkdir()
    core.git(repo2, "init", "-q", "-b", "main")
    (repo2 / core.REL_PII_TERMS).parent.mkdir(parents=True)
    (repo2 / core.REL_PII_TERMS).write_text(f"{FIXTURE_TERM}\n", encoding="utf-8")
    guide2 = selftest_guide(repo2)
    ck("결정론: 안내 픽스처도 저장소와 무관하게 같은 SHA", guide2 == guide)
    ck("★결정론: 같은 입력·주입 시각·안내 = 같은 커밋 SHA",
       commit_payload(repo2, pay, message=msg, generated_utc=_FX_UTC, guide=guide2) == c1)

    # ── 멱등 재개: 다시 불러도 같은 오브젝트 · ref 쓰기 0(재개 판단은 draft state 의 anchor 가 한다)
    refs_before = core.git_out(repo, "for-each-ref", "--format=%(refname) %(objectname)")
    ck("★결정론 SHA 재개(같은 페이로드·메시지·시각·안내 = 같은 SHA · ref 불변)",
       commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC, guide=guide) == c1
       and core.git_out(repo, "for-each-ref", "--format=%(refname) %(objectname)") == refs_before)
    ck("★다른 안내 = 다른 SHA(부모가 SHA 에 들어간다)",
       commit_payload(repo, pay, message=msg, generated_utc=_FX_UTC,
                      guide=selftest_guide(repo, body=FX_GUIDE_BODY + "개정\n")) != c1)

    # ── 앵커 게이트
    try:
        require_payload_anchor(repo, _FX_TAG, c1)
        ck("앵커 게이트 통과", True)
    except core.HintError as e:
        ck(f"앵커 게이트 통과({e.code})", False)
    ck("★소스 커밋 봉인 = HINT_ANCHOR_PARENT_NOT_GUIDE",
       expect_code(lambda: require_payload_anchor(repo, _FX_TAG, source), "HINT_ANCHOR_PARENT_NOT_GUIDE"))
    old_chain = core.git(repo, "commit-tree", (read_object_text(repo, "commit", c1) or "").split("\n", 1)[0][5:],
                         "--no-gpg-sign", "-p", old_guide, input_text=msg, env_extra=identity_env(_FX_UTC)).stdout.strip()
    ck("★마커 없는 옛 안내 위의 페이로드 커밋(v6 체인형) = HINT_ANCHOR_PARENT_NOT_GUIDE",
       expect_code(lambda: require_payload_anchor(repo, _FX_TAG, old_chain), "HINT_ANCHOR_PARENT_NOT_GUIDE"))
    merge = core.git(repo, "commit-tree", (read_object_text(repo, "commit", c1) or "").split("\n", 1)[0][5:],
                     "--no-gpg-sign", "-p", guide, "-p", source, input_text=msg,
                     env_extra=identity_env(_FX_UTC)).stdout.strip()
    ck("★부모 둘 = HINT_ANCHOR_PARENT_NOT_GUIDE", expect_code(lambda: require_payload_anchor(repo, _FX_TAG, merge),
                                                            "HINT_ANCHOR_PARENT_NOT_GUIDE"))
    ck("다른 형식(v6 마커) 안내 위 태그도 안내 위이면 유효(형식 전환 뒤 앞 형식 태그 보존)",
       code_of(lambda: require_payload_anchor(repo, _FX_TAG, core.git(
           repo, "commit-tree", (read_object_text(repo, "commit", c1) or "").split("\n", 1)[0][5:], "--no-gpg-sign",
           "-p", v6_guide, input_text=msg, env_extra=identity_env(_FX_UTC)).stdout.strip())) is None)
    ck("★남의 페이로드 = HINT_ANCHOR_PROVENANCE_TAG_MISMATCH",
       expect_code(lambda: require_payload_anchor(repo, _FX_TAG + "x", c1), "HINT_ANCHOR_PROVENANCE_TAG_MISMATCH"))
    ck("★빈 앵커 = HINT_ANCHOR_UNRESOLVED",
       expect_code(lambda: require_payload_anchor(repo, _FX_TAG, ""), "HINT_ANCHOR_UNRESOLVED"))

    # ── 두 번째 발행: 부모 = 같은 안내(체인 ✗ · PC 간 분기 원천 차단)
    tag2 = _FX_TAG.replace("len262144", "len131072")
    pay2 = tmp / "payload2"
    write_fixture_payload(pay2, tag2, source_anchor=None)
    c2 = commit_payload(repo, pay2, message=commit_message(pay2, generated_utc=_FX_UTC), generated_utc=_FX_UTC,
                        guide=guide)
    ck("★두 번째 페이로드도 부모 = 안내(직전 페이로드 ✗)",
       _commit_parents(read_object_text(repo, "commit", c2) or "") == [guide] and hint_tip(repo) is None)

    # ── ★CAS(형식 전환 전용 _update_hint_ref): 낡은 기대값으로는 옮기지 않는다 · 생성 전용 CAS
    _update_hint_ref(repo, guide, None)                           # 격리 저장소: 로컬 hint 를 안내에 둔다
    ck("★CAS 충돌 = HINT_BRANCH_CAS_CONFLICT",
       expect_code(lambda: _update_hint_ref(repo, c1, c1), "HINT_BRANCH_CAS_CONFLICT") and hint_tip(repo) == guide)
    ck("★생성 전용 CAS: 이미 있으면 거부",
       expect_code(lambda: _update_hint_ref(repo, c1, None), "HINT_BRANCH_CAS_CONFLICT") and hint_tip(repo) == guide)

    # ── ★PAYLOAD.json 없는 커밋을 브랜치에 올린 경우(배관으로 직접)
    pay3 = tmp / "payload3"
    write_fixture_payload(pay3, tag2, source_anchor=None)
    (pay3 / PAYLOAD_JSON).unlink()
    t3 = build_tree(repo, pay3, sorted(p.relative_to(pay3).as_posix() for p in pay3.rglob("*") if p.is_file()))
    c3 = core.git(repo, "commit-tree", t3, "-p", guide, "--no-gpg-sign", input_text="raw\n",
                  env_extra=identity_env(_FX_UTC)).stdout.strip()
    ck("★PAYLOAD 없는 페이로드 커밋 = HINT_ANCHOR_PAYLOAD_ABSENT",
       expect_code(lambda: require_payload_anchor(repo, tag2, c3), "HINT_ANCHOR_PAYLOAD_ABSENT"))

    # ── ★PII: 페이로드 · 메시지 · 신원 · terms 부재 (전부 ref 불변)
    tip_before = hint_tip(repo)
    tag4 = _FX_TAG.replace("len262144", "len65536")
    for label, rel, text in (
            ("리터럴 term", "02-narrative.md", f"운영 메모 {FIXTURE_TERM}\n"),
            ("사설 IPv4", "01-artifacts.md", "노드 " + ".".join(["10", "1", "2", "3"]) + " 에서 실행\n"),
            ("baking 된 운영자 경로", "artifacts/triplet/.env.m", "NAS=" + "/" + "home" + "/someone/models\n")):
        pay4 = tmp / "payload4"
        write_fixture_payload(pay4, tag4, source_anchor=None, extra_files={rel: text})
        ck(f"★페이로드 PII({label}) = HINT_PAYLOAD_PII",
           expect_code(lambda: commit_payload(repo, pay4, message=commit_message(pay4, generated_utc=_FX_UTC),
                                              generated_utc=_FX_UTC, guide=guide), "HINT_PAYLOAD_PII"))
    ck("★PII 차단 뒤 ref 불변", hint_tip(repo) == tip_before)
    pay4 = tmp / "payload4"
    write_fixture_payload(pay4, tag4, source_anchor=None)
    ck("★커밋 메시지 PII = HINT_COMMIT_MESSAGE_PII",
       expect_code(lambda: commit_payload(repo, pay4, message=f"hint: {tag4}\n{FIXTURE_TERM}\n",
                                          generated_utc=_FX_UTC, guide=guide), "HINT_COMMIT_MESSAGE_PII"))
    ck("★합성 신원이 PII 목록에 걸리면 tripwire 가 울린다",
       identity_pii_hits([core.SYNTHETIC_NAME]) != [] and identity_pii_hits([FIXTURE_TERM]) == [])
    terms_path = repo / core.REL_PII_TERMS
    terms_path.rename(tmp / "terms.bak")
    ck("★pii_terms 부재 = HINT_PII_TERMS_ABSENT(fail-closed)",
       expect_code(lambda: commit_payload(repo, pay4, message=commit_message(pay4, generated_utc=_FX_UTC),
                                          generated_utc=_FX_UTC, guide=guide), "HINT_PII_TERMS_ABSENT"))
    (tmp / "terms.bak").rename(terms_path)

    # ── ★문서 정합: ⓐ↔ⓒ · 태그 불일치 · 선언 ≠ 실물
    write_fixture_payload(pay4, tag4, source_anchor=source)
    ck("★메시지에 source_anchor 없음 = HINT_ANCHOR_TRIPLE_MISMATCH",
       expect_code(lambda: commit_payload(repo, pay4, message=f"hint: {tag4}\n", generated_utc=_FX_UTC, guide=guide),
                   "HINT_ANCHOR_TRIPLE_MISMATCH"))
    prov = json.loads((pay4 / PROVENANCE_JSON).read_text(encoding="utf-8"))
    core.write_json(pay4 / PROVENANCE_JSON, {**prov, "tag": _FX_TAG})
    ck("★PROVENANCE.tag ≠ PAYLOAD.tag = HINT_PAYLOAD_TAG_MISMATCH",
       expect_code(lambda: commit_payload(repo, pay4, message=commit_message(pay4, generated_utc=_FX_UTC),
                                          generated_utc=_FX_UTC, guide=guide), "HINT_PAYLOAD_TAG_MISMATCH"))
    core.write_json(pay4 / PROVENANCE_JSON, {**prov, "payload_files": prov["payload_files"][1:]})
    ck("★PROVENANCE.payload_files ≠ 실물 = HINT_PROVENANCE_FILES_MISMATCH",
       expect_code(lambda: commit_payload(repo, pay4, message=commit_message(pay4, generated_utc=_FX_UTC),
                                          generated_utc=_FX_UTC, guide=guide), "HINT_PROVENANCE_FILES_MISMATCH"))
    core.write_json(pay4 / PROVENANCE_JSON, {**prov, "source_anchor": source[:12]})
    ck("★PROVENANCE.source_anchor 비-전체 SHA = HINT_PROVENANCE_SHAPE",
       expect_code(lambda: commit_payload(repo, pay4, message=f"hint: {tag4}\nsource_anchor: {source[:12]}\n",
                                          generated_utc=_FX_UTC, guide=guide), "HINT_PROVENANCE_SHAPE"))
    ck("★비-UTC 시각 = HINT_TIME_NOT_INJECTED",
       expect_code(lambda: commit_payload(repo, pay, message=msg, generated_utc="2026-09-21 10:21", guide=guide),
                   "HINT_TIME_NOT_INJECTED"))

    # ── ★페이로드 문서 정합(신 형식 · 이름 재조립 · PROMPT 잔재 · 경로 이름 PII · 옛 문법 이름)
    def _commit(p: Path):
        return lambda: commit_payload(repo, p, message=commit_message(p, generated_utc=_FX_UTC), generated_utc=_FX_UTC,
                                      guide=guide)

    write_fixture_payload(pay4, tag4, source_anchor=None,
                          extra_files={"02-narrative.md": "## 2.1 출발점\n<!-- PROMPT\n질문: 무엇이 벽이었나\n-->\n"})
    ck("★PROMPT 잔재 = HINT_PROMPT_RESIDUE", expect_code(_commit(pay4), "HINT_PROMPT_RESIDUE"))
    write_fixture_payload(pay4, tag4, source_anchor=None,
                          extra_files={"01-artifacts.md": "## 1.1 적용 판정\n<<AGENT: 이 줄을 바꿔라>>\n"})
    ck("★미저작 자리표시 잔재 = HINT_AGENT_PLACEHOLDER_RESIDUE",
       expect_code(_commit(pay4), "HINT_AGENT_PLACEHOLDER_RESIDUE"))
    write_fixture_payload(pay4, tag4, source_anchor=None, payload_doc={**fixture_payload_doc(tag4), "format": "hint/v5"})
    ck("★옛 형식 표지 = HINT_PAYLOAD_FORMAT", expect_code(_commit(pay4), "HINT_PAYLOAD_FORMAT"))
    doc_no_naming = {k: v for k, v in fixture_payload_doc(tag4).items() if k != "naming"}
    write_fixture_payload(pay4, tag4, source_anchor=None, payload_doc=doc_no_naming)
    ck("★PAYLOAD.naming 부재 = HINT_PAYLOAD_NAMING_ABSENT(대조 부재 ≠ 대조 통과)",
       expect_code(_commit(pay4), "HINT_PAYLOAD_NAMING_ABSENT"))
    write_fixture_payload(pay4, tag4, source_anchor=None, payload_doc={**fixture_payload_doc(tag2), "tag": tag4})
    ck("★naming 재조립 ≠ PAYLOAD.tag = HINT_DERIVED_NAME_MISMATCH",
       expect_code(_commit(pay4), "HINT_DERIVED_NAME_MISMATCH"))
    bad_axes = fixture_payload_doc(tag4)
    bad_axes["naming"]["axes"]["len"] = {"value": "4096", "source": "fixture:tamper"}
    write_fixture_payload(pay4, tag4, source_anchor=None, payload_doc=bad_axes)
    ck("★naming 축 ≠ 세그먼트 = HINT_NAMING_INCONSISTENT", expect_code(_commit(pay4), "HINT_NAMING_INCONSISTENT"))
    write_fixture_payload(pay4, tag4, source_anchor=None, extra_files={f"artifacts/triplet/{FIXTURE_TERM}.env": "A=1\n"})
    # 경로 이름은 PROVENANCE.payload_files 에도 실리므로 내용 스캔이 먼저 잡는다 — 경로 스캔은 그 목록 규칙이 바뀌어도
    # 남는 이중 방어다. 어느 쪽이든 **차단**이 계약이다.
    ck("★경로 이름 PII 는 커밋을 막는다", code_of(_commit(pay4)) in {"HINT_PAYLOAD_PII", "HINT_PAYLOAD_PATH_PII"})
    legacy = "hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8"
    write_fixture_payload(pay4, legacy, source_anchor=None)
    ck("★옛 문법 이름(노드축 없음)은 새로 얹지 않는다 = HINT_ARCH_NODE_AXIS_ABSENT",
       expect_code(_commit(pay4), "HINT_ARCH_NODE_AXIS_ABSENT"))
    ck("거부들 뒤 ref 불변", hint_tip(repo) == tip_before)

    # ── ★로컬 hint 가 무엇을 가리키든 commit_payload 는 읽지도 옮기지도 않는다(옛 "tip 위에 얹기" · 체크아웃 가드 불요)
    write_fixture_payload(pay4, tag4, source_anchor=None)
    _update_hint_ref(repo, c2, tip_before)                       # 로컬 hint 를 엉뚱한 페이로드 커밋에 둔다(옛 체인형 로컬)
    c4 = _commit(pay4)()
    ck("★부모 = 지정 안내(로컬 hint tip 무관) · 로컬 hint 불이동",
       _commit_parents(read_object_text(repo, "commit", c4) or "") == [guide] and hint_tip(repo) == c2)
    _update_hint_ref(repo, tip_before, c2)

    # ── commit_violations: git 이 든 바이트로 같은 계약을 다시 묻는다(봉인 전·후 검사의 공용 판정)
    terms = [FIXTURE_TERM]
    ck("commit_violations(정상 페이로드 커밋) = 0", commit_violations(repo, c1, _FX_TAG, terms) == [])
    ck("★commit_violations 남의 태그 = HINT_PAYLOAD_TAG_MISMATCH",
       any(p.startswith("HINT_PAYLOAD_TAG_MISMATCH:") for p in commit_violations(repo, c1, tag2, terms)))
    ck("★commit_violations PAYLOAD 없는 커밋 = HINT_PAYLOAD_REQUIRED_ABSENT",
       any(p.startswith("HINT_PAYLOAD_REQUIRED_ABSENT:") for p in commit_violations(repo, c3, tag2, terms)))
    tree1 = (read_object_text(repo, "commit", c1) or "").split("\n", 1)[0].removeprefix("tree ")
    op = core.git(repo, "commit-tree", tree1, "--no-gpg-sign", input_text=msg,
                  env_extra={**operator_env, "GIT_AUTHOR_DATE": core.git_date(_FX_UTC),
                             "GIT_COMMITTER_DATE": core.git_date(_FX_UTC)}).stdout.strip()
    ck("★운영자 신원 커밋 = HINT_COMMIT_IDENTITY_NOT_SYNTHETIC(목록 스캔이 아니라 동일성 판정)",
       any(p.startswith("HINT_COMMIT_IDENTITY_NOT_SYNTHETIC:") for p in commit_violations(repo, op, _FX_TAG, terms)))
    # ★ 객체가 아닌 PAYLOAD.json(`[]`)이 문서 판정 전체를 끄지 못한다 — PROMPT 잔재까지 실린 손 배관 커밋(2026-09-22 리뷰)
    pay5 = tmp / "payload5"
    write_fixture_payload(pay5, tag4, source_anchor=None, payload_doc=[],
                          extra_files={"02-narrative.md": "## 2.1 출발점\n<!-- PROMPT\n질문: 무엇이 벽이었나\n-->\n"})
    t5 = build_tree(repo, pay5, payload_files(pay5))
    c5 = core.git(repo, "commit-tree", t5, "--no-gpg-sign", input_text=f"hint: {tag4}\n",
                  env_extra=identity_env(_FX_UTC)).stdout.strip()
    v5 = {p.split(":", 1)[0] for p in commit_violations(repo, c5, tag4, terms)}
    ck("★객체 아닌 PAYLOAD.json = UNREADABLE 이고 PROMPT 잔재도 여전히 잡힌다",
       {"HINT_PAYLOAD_JSON_UNREADABLE", "HINT_PROMPT_RESIDUE"} <= v5)
    ck("★commit_violations 는 리터럴 PII 도 본다",
       any(p.startswith("HINT_PAYLOAD_PII:") for p in commit_violations(repo, c1, _FX_TAG, ["지도이지 정답"])))


def _transition_selftest(tmp: Path, ck) -> None:
    """bare 원격만(네트워크 0): 원격 안내 읽기(guide_commit · 한 ref fetch) · 형식 불일치 차단 · 형식 전환(승인 필수 ·
    고아 페이로드 거부 · 정상 · 멱등 · 로컬 정리 · push 재개 · 형식 되돌림 거부)."""
    template = core.TEMPLATES_DIR / "hint-branch-README.md"
    tpl_body = template.read_text(encoding="utf-8")
    if readme_format(tpl_body) is not None:                       # 템플릿이 이미 마커를 가졌으면 본문만 쓴다
        tpl_body = tpl_body.split("\n", 1)[1]

    def mk_repo(name: str, *, marker: bool = True) -> Path:
        r = selftest_repo(tmp, name)
        target = r / _BRANCH_TEMPLATE_REL
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((guide_marker() + "\n" if marker else "") + tpl_body, encoding="utf-8")
        core.git(r, "add", _BRANCH_TEMPLATE_REL)
        core.git(r, "commit", "-qm", "tracked transition template")
        return r

    def mk_bare(name: str) -> Path:
        b = tmp / f"{name}.git"
        core.git(tmp, "init", "-q", "--bare", str(b))
        return b

    approve = {"approved_by": "픽스처 사람 발화 전사", "approved_utc": _FX_UTC}
    # ── 원격 = 마커 없는 옛 안내(6294a91 모양) · 로컬 hint = 옛 체인형(페이로드 커밋 · 태그가 붙잡음)
    repo = mk_repo("transition-repo")
    bare = mk_bare("transition-remote")
    core.git(repo, "remote", "add", "origin", str(bare))
    old = selftest_guide(repo, fmt=None)
    _update_hint_ref(repo, old, None)
    core.git(repo, "push", "-q", "origin", f"{HINT_BRANCH_REF}:{HINT_BRANCH_REF}")
    pay = tmp / "transition-payload"
    write_fixture_payload(pay, _FX_TAG, source_anchor=None)
    chain_tree = build_tree(repo, pay, payload_files(pay))
    chain = core.git(repo, "commit-tree", chain_tree, "--no-gpg-sign", "-p", old,
                     input_text=commit_message(pay, generated_utc=_FX_UTC), env_extra=identity_env(_FX_UTC)).stdout.strip()
    _update_hint_ref(repo, chain, old)                            # 로컬 hint 가 원격보다 페이로드 1커밋 앞선 옛 체인 상태
    ck("★v7 발행기: 원격 tip 이 마커 없는 옛 안내 = HINT_BRANCH_FORMAT_MISMATCH(처방 = branch-transition)",
       expect_code(lambda: guide_commit(repo, "origin"), "HINT_BRANCH_FORMAT_MISMATCH"))
    try:
        guide_commit(repo, "origin")
    except core.HintError as e:
        ck("형식 불일치 처방 문구에 branch-transition · 사람 승인", "branch-transition" in (e.remedy or "")
           and "승인" in (e.remedy or ""))
    ck("★원격 조회 실패 = HINT_REMOTE_QUERY_FAILED(fail-closed)",
       expect_code(lambda: guide_commit(repo, str(tmp / "absent.git")), "HINT_REMOTE_QUERY_FAILED"))
    empty = mk_bare("transition-empty")
    ck("★원격에 hint 브랜치 없음 = HINT_BRANCH_GUIDE_ABSENT",
       expect_code(lambda: guide_commit(repo, str(empty)), "HINT_BRANCH_GUIDE_ABSENT"))
    ck("★commit_payload(remote=) 도 같은 거부 · 로컬 hint 불변",
       expect_code(lambda: commit_payload(repo, pay, message=commit_message(pay, generated_utc=_FX_UTC),
                                          generated_utc=_FX_UTC, remote="origin"), "HINT_BRANCH_FORMAT_MISMATCH")
       and hint_tip(repo) == chain)

    # ── 승인 · 템플릿 마커 · 고아 페이로드 — 전부 ref 쓰기 0
    refs0 = core.git_out(repo, "for-each-ref", "--format=%(refname) %(objectname)")
    ck("★승인 전사 없음 = HINT_APPROVAL_ABSENT",
       expect_code(lambda: branch_transition(repo, remote="origin", generated_utc=_FX_UTC), "HINT_APPROVAL_ABSENT"))
    ck("★태그가 붙잡지 않는 로컬 페이로드 = HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD",
       expect_code(lambda: branch_transition(repo, remote="origin", generated_utc=_FX_UTC, **approve),
                   "HINT_BRANCH_TRANSITION_ORPHAN_PAYLOAD"))
    ck("거부들 뒤 ref·원격 불변", core.git_out(repo, "for-each-ref", "--format=%(refname) %(objectname)") == refs0
       and _remote_hint_tip(repo, "origin") == old)
    nomark = mk_repo("transition-nomarker", marker=False)
    core.git(nomark, "remote", "add", "origin", str(bare))
    ck("★템플릿 첫 줄에 마커 없음 = HINT_BRANCH_TEMPLATE_FORMAT(도구가 덧붙이지 않는다)",
       expect_code(lambda: branch_transition(nomark, remote="origin", generated_utc=_FX_UTC, **approve),
                   "HINT_BRANCH_TEMPLATE_FORMAT"))

    # ── 정상 전환: 태그가 옛 체인을 붙잡으면 로컬 정리 + 전환 + push
    core.git(repo, "tag", "-a", "-m", "fixture", _FX_TAG, chain)  # 격리 저장소 픽스처(옛 체인 태그)
    result = branch_transition(repo, remote="origin", generated_utc=_FX_UTC, **approve)
    new = hint_tip(repo)
    entries = tree_entries(repo, new)
    ck("branch-transition: README 하나 mode 100644 · 첫 줄 v7 마커",
       result["status"] == "transitioned" and len(entries) == 1 and entries[0]["mode"] == "100644"
       and entries[0]["path"] == GUIDE_README and readme_format(read_blob(repo, new, GUIDE_README)) == BRANCH_FORMAT)
    ck("branch-transition: 부모 = 원격의 옛 안내(로컬 체인 ✗) · remote == local",
       _commit_parents(read_object_text(repo, "commit", new) or "") == [old] and _remote_hint_tip(repo, "origin") == new)
    ck("★원격 추적 ref 도 새 안내(push 가 갱신 · 낡은 origin/hint 해소)",
       core.git(repo, "rev-parse", "--verify", "--quiet", "refs/remotes/origin/hint", check=False).stdout.strip() == new)
    ck("★옛 체인 태그는 여전히 자기 커밋을 든다(R6)", core.git_out(repo, "rev-parse", f"{_FX_TAG}^{{commit}}") == chain)
    ck("★승인 전사는 반환값에만(배포 오브젝트 ✗)", result.get("approved_by") == approve["approved_by"]
       and approve["approved_by"].encode() not in (core.git_bytes(repo, "cat-file", "commit", new) or b""))
    ck("branch-transition: 재실행 = already-transitioned(멱등 · 재시도 UTC 무관)",
       branch_transition(repo, remote="origin", generated_utc="2026-09-22T00:00:00Z", **approve)["status"]
       == "already-transitioned" and hint_tip(repo) == new)
    ck("전환 뒤 guide_commit(origin) = (새 안내, v7)", guide_commit(repo, "origin") == (new, BRANCH_FORMAT))

    # ── 다른 클론: 안내 오브젝트가 로컬에 없으면 그 한 ref 만 fetch · 로컬 브랜치·태그 불이동
    clone = selftest_repo(tmp, "transition-clone")
    core.git(clone, "remote", "add", "origin", str(bare))
    ck("대조: 클론에는 안내 오브젝트가 없다",
       core.git(clone, "cat-file", "-e", f"{new}^{{commit}}", check=False).returncode != 0)
    refs_c = core.git_out(clone, "for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/tags")
    ck("★guide_commit: 한 ref fetch 뒤 (새 안내, v7)", guide_commit(clone, "origin") == (new, BRANCH_FORMAT))
    ck("★fetch 는 로컬 브랜치·태그를 만들지 않는다(추적 ref 만)",
       core.git_out(clone, "for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/tags") == refs_c
       and hint_tip(clone) is None
       and core.git(clone, "rev-parse", "--verify", "--quiet", "refs/remotes/origin/hint", check=False).stdout.strip() == new)
    clone2 = selftest_repo(tmp, "transition-clone-url")
    ck("URL·경로 원격(추적 ref 없음)도 오브젝트만 받는다", guide_commit(clone2, str(bare)) == (new, BRANCH_FORMAT)
       and core.git_out(clone2, "for-each-ref", "--format=%(refname)", "refs/remotes") == "")
    pay_c = tmp / "transition-clone-payload"
    write_fixture_payload(pay_c, _FX_TAG, source_anchor=None)
    cc = commit_payload(clone, pay_c, message=commit_message(pay_c, generated_utc=_FX_UTC), generated_utc=_FX_UTC,
                        remote="origin")
    ck("★commit_payload(remote=): 부모 = 원격 안내 · 로컬 hint 불이동",
       _commit_parents(read_object_text(clone, "commit", cc) or "") == [new] and hint_tip(clone) is None)
    ck("★guide= 와 remote= 가 다르면 = HINT_BRANCH_GUIDE_MOVED",
       expect_code(lambda: commit_payload(clone, pay_c, message=commit_message(pay_c, generated_utc=_FX_UTC),
                                          generated_utc=_FX_UTC, guide=selftest_guide(clone), remote="origin"),
                   "HINT_BRANCH_GUIDE_MOVED"))

    # ── 로컬 정리: 원격은 이미 v7 · 로컬 hint 만 옛 값 → local-synced
    stale = mk_repo("transition-stale")
    core.git(stale, "remote", "add", "origin", str(bare))
    _update_hint_ref(stale, selftest_guide(stale, fmt=None), None)
    synced = branch_transition(stale, remote="origin", generated_utc=_FX_UTC, **approve)
    ck("★원격 v7 · 로컬 옛 값 = local-synced(로컬만 원격 tip 으로 · push 0)",
       synced["status"] == "local-synced" and hint_tip(stale) == new and _remote_hint_tip(stale, "origin") == new)

    # ── push 재개: 로컬 = 전환 후보(부모 = 원격 tip) · 원격 = 옛 값
    repo2 = mk_repo("transition-resume")
    bare2 = mk_bare("transition-resume")
    core.git(repo2, "remote", "add", "origin", str(bare2))
    old2 = selftest_guide(repo2, fmt=None)
    _update_hint_ref(repo2, old2, None)
    core.git(repo2, "push", "-q", "origin", f"{HINT_BRANCH_REF}:{HINT_BRANCH_REF}")
    staged = _transition_commit(repo2, old2, _transition_tree(repo2, _transition_template_bytes(repo2)), _FX_UTC)
    _update_hint_ref(repo2, staged, old2)
    resumed = branch_transition(repo2, remote="origin", generated_utc="2026-09-22T00:00:00Z", **approve)
    ck("branch-transition: local=후보 remote=old → push-resumed(재시도 UTC 달라도 구조 판정)",
       resumed["status"] == "push-resumed" and _remote_hint_tip(repo2, "origin") == staged)

    # ── 형식 되돌림 거부: 원격이 더 새 형식(v8)
    repo3 = mk_repo("transition-downgrade")
    bare3 = mk_bare("transition-downgrade")
    core.git(repo3, "remote", "add", "origin", str(bare3))
    v8 = selftest_guide(repo3, fmt="v8")
    _update_hint_ref(repo3, v8, None)
    core.git(repo3, "push", "-q", "origin", f"{HINT_BRANCH_REF}:{HINT_BRANCH_REF}")
    ck("★원격 v8 안내 = HINT_BRANCH_TRANSITION_DOWNGRADE · 원격 불변",
       expect_code(lambda: branch_transition(repo3, remote="origin", generated_utc=_FX_UTC, **approve),
                   "HINT_BRANCH_TRANSITION_DOWNGRADE") and _remote_hint_tip(repo3, "origin") == v8)
    ck("★v7 발행기에게 v8 안내도 형식 불일치", expect_code(lambda: guide_commit(repo3, "origin"), "HINT_BRANCH_FORMAT_MISMATCH"))
    # ── 원격 tip 이 페이로드 커밋(옛 체인을 누가 밀었다) → 안내가 아니다
    repo4 = mk_repo("transition-payloadtip")
    bare4 = mk_bare("transition-payloadtip")
    pay4 = tmp / "transition-payload4"
    write_fixture_payload(pay4, _FX_TAG, source_anchor=None)
    ptip = core.git(repo4, "commit-tree", build_tree(repo4, pay4, payload_files(pay4)), "--no-gpg-sign",
                    input_text="x\n", env_extra=identity_env(_FX_UTC)).stdout.strip()
    _update_hint_ref(repo4, ptip, None)
    core.git(repo4, "push", "-q", str(bare4), f"{HINT_BRANCH_REF}:{HINT_BRANCH_REF}")
    ck("★원격 tip = 페이로드 커밋 = HINT_BRANCH_FORMAT_MISMATCH(트리가 README 하나가 아님)",
       expect_code(lambda: guide_commit(repo4, str(bare4)), "HINT_BRANCH_FORMAT_MISMATCH"))
