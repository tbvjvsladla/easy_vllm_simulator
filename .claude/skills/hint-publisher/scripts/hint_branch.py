#!/usr/bin/env python3
"""hint_branch.py — hint 브랜치 페이로드 트리의 생애주기 (plan_26090107 Phase 2).

무엇을 하나
    hint 태그가 가리키는 커밋의 **트리를 짓는다**. 태그 본문(annotation)은 `hint_tag.py` 가
    계속 소유하고, 이 스크립트는 그 태그가 **배포하는 것**(archive = 트리)을 소유한다.
    둘의 경계가 곧 감사 `audit_26090118` N-3 이 지적한 공백이다 —
    "게이트는 본문을 지키도록 설계됐는데 새 아키텍처는 트리를 배포한다."

★ 설계: 워킹트리를 쓰지 않는다 (plan D1.2 개정 · 2026-09-01)
    계획 초안은 `git worktree` + 빈 인덱스로 "비우고 다시 짓기"를 처방했다. 여기서는
    **순수 배관**(hash-object → mktree → commit-tree)만 쓴다. 워킹트리가 **아예 없으므로**:
      C1 잔존물 오염      → 트리를 매번 목록에서 새로 짓는다. 넣지 않은 것은 들어갈 수 없다.
      C2 브랜치 전환 파괴 → 체크아웃이 0회다. 메인 워킹트리는 관측조차 되지 않는다.
      C3 dirty clear 소실 → 지우는 동작 자체가 없다.
    즉 세 위험이 **가드로 막히는 것이 아니라 구조적으로 불가능**해진다. 계획이 요구한
    "성공을 선언하지 않고 결과를 검사한다"(C4)는 그대로 유지한다 — §verify_tree.

    dirty 검사는 남는다. 다만 **파괴 방지가 아니라 증명(provenance) 게이트**로 성격이 바뀐다:
    앵커는 커밋 SHA 인데 페이로드가 미커밋 워킹트리에서 왔다면 **앵커가 페이로드를 증명하지
    못한다**. 그 불일치를 막는 것이 --require-clean 이다.

앵커 3중 기록 (plan D1)
    ⓐ hint 커밋 메시지  ⓑ annotated 태그 footer  ⓒ 페이로드 PROVENANCE.json
    셋이 **모두 같아야** 통과한다. 하나만 기록하면 그 하나가 틀렸을 때 아무도 모른다.

서브커맨드
    build-tree    페이로드 디렉터리 + 파일목록 → 트리 오브젝트 (stdout: sha)
    verify-tree   트리 전수 == 목록 완전일치 (초과 0 · 부족 0)
    publish       게이트 전량 → commit-tree → refs/heads/hint 전진 (태그는 hint_tag 소관)
    --self-test   음성대조 포함 회귀
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import shutil
import tempfile
from pathlib import Path

HINT_BRANCH = "refs/heads/hint"
PROVENANCE_NAME = "PROVENANCE.json"
# 페이로드에 절대 들어가면 안 되는 이름. 하드코딩이지만 tripwire 칸에 해당한다 —
# 닫힌 목록이고, 여기 손대려면 리뷰가 강제된다(workflow.md 안티패턴 판정표).
FORBIDDEN_TOP = {".git", ".claude", "__pycache__"}

# `${VAR:-/some/path}` 의 기본값을 가려내는 **구조적 판별자**. 값의 모양이 아니라 **문맥**으로
# 가른다(`_SECTION_ANCHOR` 가 §10.1 을 사설 IP 와 가르는 것과 같은 규율). compose 는 경로를
# baking 하지 않고 치환형으로만 쓰므로, 그 기본값은 운영자 지문이 아니라 폴백이다.
# 좁게 잡는다 — 매치 **직전** 텍스트가 `${…:-` 로 끝나야만 면제한다.
_SHELL_DEFAULT = re.compile(r"\$\{[A-Za-z_][A-Za-z0-9_]*:-$")


def die(msg: str, code: int = 2) -> None:
    print(f"[hint_branch] {msg}", file=sys.stderr)
    raise SystemExit(code)


def git(*args: str, cwd: Path | None = None, input_bytes: bytes | None = None) -> str:
    """git 를 부르고 stdout 을 준다. 실패는 조용히 삼키지 않는다."""
    proc = subprocess.run(["git", *args], cwd=str(cwd) if cwd else None,
                          input=input_bytes, capture_output=True)
    if proc.returncode != 0:
        die(f"git {' '.join(args)} 실패(rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}")
    return proc.stdout.decode("utf-8", "replace")


# ---------------------------------------------------------------- PII

def _load_pii():
    """정본 4종 패턴 + 리터럴을 hint_tag 에서 **가져온다**(복사하지 않는다).

    감사 N-4: 트리를 보는 스캐너가 정본과 다른 패턴 집합을 들고 있어 배포 클론에서 4종 중
    1종만 살았다. 여기서는 그 실수를 되풀이하지 않는다 — 정본이 바뀌면 이쪽도 함께 바뀐다.
    """
    here = Path(__file__).resolve().parent
    sys.path.insert(0, str(here))
    try:
        import hint_tag  # noqa: PLC0415
    except Exception as exc:  # pragma: no cover - 배선 사고
        die(f"정본 PII 패턴을 hint_tag 에서 가져오지 못했다: {exc!r} — fail-closed")
    return hint_tag.GENERIC_PII, hint_tag.load_pii_terms(), getattr(hint_tag, "_SECTION_ANCHOR", None)


def scan_payload_pii(root: Path, rels: list[str]) -> list[str]:
    """페이로드 전량을 정본 4종 + 리터럴로 스캔한다. 배포면이므로 면제가 없다.

    plan R1: 미추적이던 파일(devlog·.env)이 배포면으로 올라오므로 **4종 전부 fail-closed**.
    """
    generic, terms, section_anchor = _load_pii()
    if terms is None:
        die("`.claude/pii_terms.txt` 부재 — 페이로드 PII-clean 인증 불가(fail-closed). "
            "감사 ⑫: 이 파일은 비추적이라 배포 클론의 기본 상태가 부재다.")
    hits: list[str] = []
    exempt: list[str] = []
    for rel in rels:
        p = root / rel
        try:
            text = p.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue  # 바이너리 — 확장자가 아니라 내용으로 판정(감사 N-1 과 같은 규율)
        for lineno, line in enumerate(text.splitlines(), 1):
            for term in terms:
                if term and term in line:
                    hits.append(f"{rel}:{lineno}: literal:{term}")
            for name, pat in generic:
                for m in pat.finditer(line):
                    if (name == "private-ipv4" and section_anchor is not None
                            and section_anchor.search(line[:m.start()])):
                        continue  # 문서 절번호(§10.1) 는 사설 IP 가 아니다
                    if name == "abs-op-path" and _SHELL_DEFAULT.search(line[:m.start()]):
                        # `${VAR:-/mnt/models}` 의 기본값은 **운영자 경로가 아니라 폴백**이다.
                        # 실값은 env 로 주입되며 그 env 는 배포되지 않는다(형상 템플릿만 나간다).
                        # 다만 **조용히 넘기지 않는다** — 무엇을 면제했는지 출력한다.
                        exempt.append(f"{rel}:{lineno}: shell-default:{m.group(0)[:48]}")
                        continue
                    hits.append(f"{rel}:{lineno}: {name}:{m.group(0)[:48]}")
    for e in exempt:
        print(f"[hint_branch] EXEMPT(shell-default) {e}", file=sys.stderr)
    return hits


# ---------------------------------------------------------------- 트리 짓기

def _read_manifest(path: Path) -> list[str]:
    """페이로드 파일 목록(allowlist). 한 줄 하나, '#' 주석, 빈 줄 무시."""
    rels: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("/") or ".." in Path(line).parts:
            die(f"목록에 절대경로/상위탈출이 있다: {line!r} — 거부")
        rels.append(line)
    dupes = {r for r in rels if rels.count(r) > 1}
    if dupes:
        die(f"목록에 중복이 있다: {sorted(dupes)}")
    if not rels:
        die("페이로드 목록이 비었다 — 빈 트리를 발행하지 않는다")
    return rels


def _mode_for(path: Path) -> str:
    if path.is_symlink():
        die(f"심링크는 페이로드에 담지 않는다: {path} — 대상이 트리 밖을 가리킬 수 있다")
    return "100755" if os.access(path, os.X_OK) else "100644"


def build_tree(root: Path, rels: list[str], repo: Path) -> str:
    """allowlist 로 트리를 **새로 짓는다**. 빠뜨린 것은 들어갈 수 없다(C1)."""
    for rel in rels:
        top = Path(rel).parts[0]
        if top in FORBIDDEN_TOP:
            die(f"금지된 최상위 항목이 목록에 있다: {rel!r} "
                f"(합격기준 A1 — archive 에 프로젝트 코드를 담지 않는다)")
        p = root / rel
        if not p.is_file():
            die(f"목록에 있으나 실물이 없다: {rel} — 부재를 성공으로 넘기지 않는다")

    # 디렉터리별로 모아 바닥부터 mktree 한다.
    children: dict[str, list[tuple[str, str, str, str]]] = {}  # dir -> (mode,type,sha,name)
    for rel in rels:
        p = root / rel
        sha = git("hash-object", "-w", "--", str(p), cwd=repo).strip()
        parent = str(Path(rel).parent) if Path(rel).parent != Path(".") else ""
        children.setdefault(parent, []).append((_mode_for(p), "blob", sha, Path(rel).name))

    # 깊은 디렉터리부터 접어 올린다.
    # ⚠ 한 번의 for 로는 안 된다 — 접는 도중에 **새 중간 디렉터리가 생긴다**
    #   (`artifacts/triplet` 을 접으면 `artifacts` 가 새로 나타난다). 미리 뽑아둔 목록으로
    #   돌리면 2단 이상 중첩에서 접기가 미완결로 끝난다(2026-09-01 실물에서 검출 —
    #   자체검사 픽스처가 1단뿐이라 못 봤고, 아래 §_run_self_test 에 2단 회귀를 넣었다).
    while any(d for d in children if d):
        deepest = max((d for d in children if d), key=lambda x: x.count("/"))
        entries = children.pop(deepest)
        payload = "".join(f"{m} {t} {s}\t{n}\n" for m, t, s, n in sorted(entries, key=lambda e: e[3]))
        sha = git("mktree", cwd=repo, input_bytes=payload.encode("utf-8")).strip()
        parent = str(Path(deepest).parent) if Path(deepest).parent != Path(".") else ""
        children.setdefault(parent, []).append(("040000", "tree", sha, Path(deepest).name))

    root_entries = children.pop("", [])
    if children:  # 접기가 끝났는데 남았다면 배선 사고다 — 조용히 넘기지 않는다
        die(f"트리 접기가 완결되지 않았다(남은 디렉터리 {sorted(children)}) — 내부 오류")
    payload = "".join(f"{m} {t} {s}\t{n}\n" for m, t, s, n in sorted(root_entries, key=lambda e: e[3]))
    return git("mktree", cwd=repo, input_bytes=payload.encode("utf-8")).strip()


def tree_paths(tree: str, repo: Path) -> list[str]:
    out = git("ls-tree", "-r", "--name-only", tree, cwd=repo)
    return sorted(x for x in out.split("\n") if x.strip())


def verify_tree(tree: str, rels: list[str], repo: Path) -> tuple[list[str], list[str]]:
    """**선언이 아니라 결과를 검사한다**(plan C4). 초과분·부족분을 각각 돌려준다."""
    actual, expected = set(tree_paths(tree, repo)), set(rels)
    return sorted(actual - expected), sorted(expected - actual)


# ---------------------------------------------------------------- 앵커 3중

ANCHOR_RE = re.compile(r"\b[0-9a-f]{40}\b")


def make_provenance(anchor: str, tag: str, source_branch: str, extra: dict | None = None) -> str:
    doc = {
        "schema_version": 1,
        "kind": "hint_payload_provenance",
        "tag": tag,
        "anchor": anchor,
        "source_branch": source_branch,
    }
    if extra:
        doc.update(extra)
    return json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def verify_anchor_triple(anchor: str, commit_message: str, provenance_text: str,
                         tag_footer: str | None) -> list[str]:
    """ⓐ커밋메시지 ⓑ태그footer ⓒPROVENANCE 가 **모두** 같은 앵커를 말하는지.

    하나만 기록하면 그 하나가 틀렸을 때 대조할 상대가 없다. footer 는 발행 시점에
    hint_tag 가 쓰므로 이 단계에서는 None 일 수 있다 — 그때는 두 축만 본다.
    """
    problems: list[str] = []
    if not ANCHOR_RE.fullmatch(anchor):
        problems.append(f"앵커가 40-hex 가 아니다: {anchor!r}")
    if anchor not in commit_message:
        problems.append("ⓐ 커밋 메시지에 앵커가 없다")
    try:
        prov = json.loads(provenance_text)
    except Exception as exc:
        problems.append(f"ⓒ PROVENANCE 파싱 실패: {exc!r}")
    else:
        if prov.get("anchor") != anchor:
            problems.append(f"ⓒ PROVENANCE.anchor={prov.get('anchor')!r} ≠ {anchor!r}")
    if tag_footer is not None and anchor not in tag_footer:
        problems.append("ⓑ 태그 footer 에 앵커가 없다")
    return problems


# ---------------------------------------------------------------- publish

def cmd_publish(a) -> int:
    repo = Path(a.repo).resolve()
    payload = Path(a.payload).resolve()
    rels = _read_manifest(Path(a.manifest))

    anchor = git("rev-parse", a.anchor + "^{commit}", cwd=repo).strip()
    source_branch = git("rev-parse", "--abbrev-ref", "HEAD", cwd=repo).strip()

    # 앵커가 HEAD 와 다르면 **깨끗한 트리**여도 페이로드는 앵커의 트리가 아니다.
    # 정당한 경우가 있다 — 서빙은 옛 커밋 상태에서 했고 페이로드는 나중에 조립한다.
    # 그래서 막지 않되 **침묵하지도 않는다**: 출처를 표시한다(헌법 §결정론 규율).
    head = git("rev-parse", "HEAD", cwd=repo).strip()
    anchor_is_head = (anchor == head)
    if not anchor_is_head:
        print(f"[hint_branch] 주의: anchor({anchor[:12]}) ≠ HEAD({head[:12]}). "
              "페이로드는 HEAD 워킹트리에서 조립됐고 앵커는 그 산출물을 **만든** 상태를 가리킨다. "
              "둘 다 PROVENANCE 에 기록한다.", file=sys.stderr)

    if a.require_clean:
        dirty = git("status", "--porcelain", cwd=repo).strip()
        if dirty:
            die("워킹트리가 dirty 다 — 앵커(커밋 SHA)가 페이로드를 증명하지 못한다.\n"
                "  파괴 방지가 아니라 **증명 게이트**다: 미커밋 상태에서 뽑은 산출물은\n"
                "  그 앵커를 체크아웃해도 재현되지 않는다.\n" + dirty)

    # ⓒ PROVENANCE 를 페이로드 안에 만들고, 그 자체를 목록에 넣는다(트리에 들어가야 배포된다)
    prov_text = make_provenance(anchor, a.tag, source_branch,
                                {"payload_files": len(rels), "generated_kst": a.generated_kst,
                                 "assembled_at_head": head,
                                 "anchor_is_head": anchor_is_head})
    prov_path = payload / PROVENANCE_NAME
    if a.dry_run:
        rels_full = rels
    else:
        prov_path.write_text(prov_text, encoding="utf-8")
        rels_full = sorted(set(rels) | {PROVENANCE_NAME})

    # A6 경고 도달 — 경고 README 는 **잊을 수 없어야 한다**(plan D1.3 · §12 A6).
    # 페이로드 조립자가 넣기를 기대하지 않고, 트리를 소유한 이쪽이 브랜치 tip 에서 끌어와
    # 항상 심는다. 그래야 브랜치 열람과 태그 zip **양쪽**에 동시에 도달한다.
    if "README.md" not in rels_full:
        tip0 = subprocess.run(["git", "rev-parse", "--verify", "--quiet", f"{HINT_BRANCH}:README.md"],
                              cwd=str(repo), capture_output=True, text=True).stdout.strip()
        if not tip0:
            die(f"{HINT_BRANCH} 에 README.md 가 없다 — 경고문이 없는 페이로드는 발행하지 않는다"
                "(plan §12 A6). 브랜치를 먼저 신설하라(Phase 0c).")
        readme = subprocess.run(["git", "cat-file", "blob", tip0],
                                cwd=str(repo), capture_output=True).stdout
        if not a.dry_run:
            (payload / "README.md").write_bytes(readme)
            rels_full = sorted(set(rels_full) | {"README.md"})
        else:
            print("[hint_branch] (dry-run) 경고 README 를 브랜치 tip 에서 심을 예정 — "
                  f"blob {tip0[:12]}")

    message = Path(a.message).read_text(encoding="utf-8")
    problems = verify_anchor_triple(anchor, message, prov_text, None)
    if problems:
        die("앵커 3중 기록 불일치:\n  " + "\n  ".join(problems))

    hits = scan_payload_pii(payload, rels_full)
    if hits:
        die(f"페이로드 PII 검출 {len(hits)}건 (배포면 = 4종 전부 fail-closed):\n  "
            + "\n  ".join(hits[:20]))

    tree = build_tree(payload, rels_full, repo)
    extra, missing = verify_tree(tree, rels_full, repo)
    if extra or missing:
        die(f"트리 전수 대조 실패 — 초과 {extra} · 부족 {missing}")

    print(f"[hint_branch] tree      {tree}  ({len(rels_full)} 파일)")
    print(f"[hint_branch] anchor    {anchor}  (branch={source_branch})")
    if a.dry_run:
        print("[hint_branch] DRY-RUN — 커밋/ref 갱신을 하지 않았다.")
        return 0

    parent = []
    tip = subprocess.run(["git", "rev-parse", "--verify", "--quiet", HINT_BRANCH],
                         cwd=str(repo), capture_output=True, text=True).stdout.strip()
    if tip:
        parent = ["-p", tip]
    commit = git("commit-tree", tree, *parent, cwd=repo,
                 input_bytes=message.encode("utf-8")).strip()
    git("update-ref", HINT_BRANCH, commit, *( [tip] if tip else [] ), cwd=repo)
    print(f"[hint_branch] commit    {commit}")
    print(f"[hint_branch] {HINT_BRANCH} → {commit[:12]}"
          + (f" (parent {tip[:12]})" if tip else " (첫 커밋)"))
    print("[hint_branch] 태그 생성은 hint_tag 소관이다 — 이 스크립트는 태그를 만들지 않는다.")
    return 0


def cmd_build_tree(a) -> int:
    repo = Path(a.repo).resolve()
    rels = _read_manifest(Path(a.manifest))
    print(build_tree(Path(a.payload).resolve(), rels, repo))
    return 0


def cmd_verify_tree(a) -> int:
    repo = Path(a.repo).resolve()
    rels = _read_manifest(Path(a.manifest))
    extra, missing = verify_tree(a.tree, rels, repo)
    if extra or missing:
        print(f"[hint_branch] FAIL — 초과 {extra} · 부족 {missing}", file=sys.stderr)
        return 1
    print(f"[hint_branch] PASS — 트리 {len(rels)} 파일이 목록과 완전일치")
    return 0


# ---------------------------------------------------------------- self-test

def _run_self_test() -> int:
    checks: list[tuple[str, bool]] = []

    def ck(name: str, cond: bool) -> None:
        checks.append((name, bool(cond)))

    def expect_die(fn) -> str | None:
        """SystemExit 를 기대한다. 안 나면 None → 시험 실패로 잡힌다."""
        try:
            fn()
        except SystemExit:
            return "raised"
        return None

    with tempfile.TemporaryDirectory() as tmp:
        tmpd = Path(tmp)
        repo = tmpd / "repo"
        repo.mkdir()
        git("init", "-q", "-b", "main", str(repo))
        git("config", "user.email", "selftest@example.invalid", cwd=repo)
        git("config", "user.name", "selftest", cwd=repo)
        (repo / "seed.txt").write_text("x\n", encoding="utf-8")
        git("add", "-A", cwd=repo)
        git("commit", "-qm", "seed", cwd=repo)
        anchor = git("rev-parse", "HEAD", cwd=repo).strip()

        pay = tmpd / "payload"
        (pay / "configs").mkdir(parents=True)
        (pay / "README.md").write_text("# hint\n", encoding="utf-8")
        (pay / "configs" / "m.yaml").write_text("model: demo\n", encoding="utf-8")
        runner = pay / "configs" / "run.sh"
        runner.write_text("#!/bin/sh\necho hi\n", encoding="utf-8")
        runner.chmod(0o755)

        man = tmpd / "files.txt"
        man.write_text("README.md\nconfigs/m.yaml\nconfigs/run.sh\n", encoding="utf-8")
        rels = _read_manifest(man)
        ck("목록 파싱 3건", rels == ["README.md", "configs/m.yaml", "configs/run.sh"])

        tree = build_tree(pay, rels, repo)
        ck("중첩 디렉터리 트리 생성", bool(re.fullmatch(r"[0-9a-f]{40}", tree)))
        ck("트리 전수 == 목록", verify_tree(tree, rels, repo) == ([], []))

        # ── 2026-09-01 실물 회귀: 2단 이상 중첩에서 트리 접기가 미완결이었다
        (pay / "artifacts" / "triplet").mkdir(parents=True)
        (pay / "artifacts" / "triplet" / "a.yaml").write_text("a: 1\n", encoding="utf-8")
        (pay / "artifacts" / "build" / "deep" / "x").mkdir(parents=True)
        (pay / "artifacts" / "build" / "deep" / "x" / "b.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        deep_rels = rels + ["artifacts/triplet/a.yaml", "artifacts/build/deep/x/b.sh"]
        deep_tree = build_tree(pay, deep_rels, repo)
        ck("★회귀 2단 중첩 트리", verify_tree(deep_tree, deep_rels, repo) == ([], []))
        ck("★회귀 4단 중첩 경로 보존",
           "artifacts/build/deep/x/b.sh" in tree_paths(deep_tree, repo))
        shutil.rmtree(pay / "artifacts")

        modes = git("ls-tree", "-r", tree, cwd=repo)
        ck("실행비트 보존(run.sh=100755)", "100755" in modes and "configs/run.sh" in modes)
        ck("일반파일은 100644", "100644 blob" in modes)

        # ── 음성대조 ① 초과분: 트리에 있는데 목록에 없다
        extra, missing = verify_tree(tree, ["README.md", "configs/m.yaml"], repo)
        ck("★음성대조 초과분 검출", extra == ["configs/run.sh"] and missing == [])

        # ── 음성대조 ② 부족분: 목록에 있는데 트리에 없다
        extra2, missing2 = verify_tree(tree, rels + ["configs/ghost.yaml"], repo)
        ck("★음성대조 부족분 검출", missing2 == ["configs/ghost.yaml"] and extra2 == [])

        # ── 음성대조 ③ 찌꺼기 1건(plan A5-ⓑ · C6): 이전 발행 잔재가 페이로드에 남았다
        (pay / "leftover.md").write_text("이전 모델 잔재\n", encoding="utf-8")
        tree_dirty = build_tree(pay, rels + ["leftover.md"], repo)
        e3, m3 = verify_tree(tree_dirty, rels, repo)
        ck("★음성대조 찌꺼기 검출(C6)", e3 == ["leftover.md"])
        ck("★찌꺼기는 목록에 없으면 트리에 못 들어간다(C1)",
           "leftover.md" not in tree_paths(build_tree(pay, rels, repo), repo))
        (pay / "leftover.md").unlink()

        # ── 음성대조 ④ 금지 최상위(A1): .claude 를 담으려 하면 거부
        (pay / ".claude").mkdir()
        (pay / ".claude" / "x.md").write_text("코드\n", encoding="utf-8")
        ck("★음성대조 .claude 거부(A1)",
           expect_die(lambda: build_tree(pay, [".claude/x.md"], repo)) == "raised")

        # ── 음성대조 ⑤ 목록에 있으나 실물 부재 → 부재를 성공으로 넘기지 않는다
        ck("★음성대조 실물부재 거부",
           expect_die(lambda: build_tree(pay, ["nope.md"], repo)) == "raised")

        # ── 음성대조 ⑥ 절대경로/상위탈출 목록 거부
        bad = tmpd / "bad.txt"
        bad.write_text("../escape.md\n", encoding="utf-8")
        ck("★음성대조 상위탈출 거부", expect_die(lambda: _read_manifest(bad)) == "raised")
        bad.write_text("/etc/passwd\n", encoding="utf-8")
        ck("★음성대조 절대경로 거부", expect_die(lambda: _read_manifest(bad)) == "raised")
        bad.write_text("# 주석만\n\n", encoding="utf-8")
        ck("★음성대조 빈 목록 거부", expect_die(lambda: _read_manifest(bad)) == "raised")
        bad.write_text("a.md\na.md\n", encoding="utf-8")
        ck("★음성대조 중복 거부", expect_die(lambda: _read_manifest(bad)) == "raised")

        # ── 앵커 3중
        prov = make_provenance(anchor, "hint/x/y/z", "main")
        msg_ok = f"payload\n\nanchor: {anchor}\n"
        ck("앵커 3중 일치 통과", verify_anchor_triple(anchor, msg_ok, prov, None) == [])
        ck("★음성대조 ⓐ커밋메시지 누락 검출",
           any("ⓐ" in p for p in verify_anchor_triple(anchor, "payload\n", prov, None)))
        other = "0" * 40
        ck("★음성대조 ⓒPROVENANCE 불일치 검출",
           any("ⓒ" in p for p in
               verify_anchor_triple(anchor, msg_ok, make_provenance(other, "t", "main"), None)))
        ck("★음성대조 ⓑfooter 누락 검출",
           any("ⓑ" in p for p in verify_anchor_triple(anchor, msg_ok, prov, "footer 없음")))
        ck("★음성대조 비-40hex 앵커 거부",
           any("40-hex" in p for p in verify_anchor_triple("deadbeef", msg_ok, prov, None)))

        # ── PII: 정본 4종을 실제로 가져오는가 + 페이로드에서 잡는가
        try:
            generic, terms, _ = _load_pii()
            names = [n for n, _ in generic]
            ck("정본 PII 4종을 hint_tag 에서 수입", set(names) ==
               {"private-ipv4", "email", "abs-op-path", "spark-host"})
            if terms is not None:
                (pay / "leak.md").write_text("경로: /home/someone/secret\n", encoding="utf-8")
                hits = scan_payload_pii(pay, ["leak.md"])
                ck("★음성대조 페이로드 PII 검출(abs-op-path)",
                   any("abs-op-path" in h for h in hits))
                (pay / "leak.md").write_text("정상 문서\n", encoding="utf-8")
                ck("무해 페이로드는 통과", scan_payload_pii(pay, ["leak.md"]) == [])
                # ── shell 기본값 판별자 (2026-09-01): compose 의 `${VAR:-/mnt/models}` 는
                #    운영자 경로가 아니라 폴백이다. 좁게(직전 텍스트로만) 가른다.
                (pay / "leak.md").write_text(
                    "- ${NAS_MODEL_PATH:-/mnt/models}:/app/models:ro\n", encoding="utf-8")
                ck("★shell 기본값은 면제된다", scan_payload_pii(pay, ["leak.md"]) == [])
                (pay / "leak.md").write_text(
                    "NAS_MODEL_PATH=/mnt/fixture-nas/Model/real\n", encoding="utf-8")
                ck("★음성대조 baking 된 실경로는 여전히 잡힌다",
                   any("abs-op-path" in h for h in scan_payload_pii(pay, ["leak.md"])))
                (pay / "leak.md").write_text(
                    "설명: ${VAR} 뒤에 /mnt/fixture-nas/Model/real 이 있다\n", encoding="utf-8")
                ck("★음성대조 치환구문 근처라도 기본값이 아니면 잡힌다",
                   any("abs-op-path" in h for h in scan_payload_pii(pay, ["leak.md"])))
                (pay / "leak.md").write_text("정상 문서\n", encoding="utf-8")
                (pay / "leak.md").unlink()
            else:
                ck("pii_terms 부재 — 스캔 시험 생략(배포 클론 기본 상태)", True)
        except SystemExit:
            ck("정본 PII 수입", False)

    failed = [n for n, ok in checks if not ok]
    for name, ok in checks:
        print(f"  {'ok  ' if ok else 'FAIL'} {name}")
    print(f"[hint_branch] self-test {len(checks) - len(failed)}/{len(checks)} "
          + ("PASS" if not failed else "FAIL"))
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description="hint 브랜치 페이로드 트리 생애주기")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo", default=".", help="저장소 루트 (기본: 현재 디렉터리)")
    sub = ap.add_subparsers(dest="cmd")

    b = sub.add_parser("build-tree", help="allowlist 로 트리 생성 (stdout: sha)")
    b.add_argument("--payload", required=True)
    b.add_argument("--manifest", required=True)
    b.set_defaults(fn=cmd_build_tree)

    v = sub.add_parser("verify-tree", help="트리 전수 == 목록 완전일치 검사")
    v.add_argument("--tree", required=True)
    v.add_argument("--manifest", required=True)
    v.set_defaults(fn=cmd_verify_tree)

    p = sub.add_parser("publish", help="게이트 전량 → commit-tree → refs/heads/hint 전진")
    p.add_argument("--payload", required=True)
    p.add_argument("--manifest", required=True)
    p.add_argument("--tag", required=True, help="이 페이로드가 받을 태그 이름(PROVENANCE 에 기록)")
    p.add_argument("--anchor", required=True, help="소스 커밋 ref")
    p.add_argument("--message", required=True, help="커밋 메시지 파일 (앵커를 포함해야 한다)")
    p.add_argument("--generated-kst", required=True, help="시각은 주입만 받는다(벽시계 금지)")
    p.add_argument("--require-clean", action="store_true", default=True)
    p.add_argument("--allow-dirty", dest="require_clean", action="store_false",
                   help="증명 게이트를 끈다 — 앵커가 페이로드를 증명하지 못함을 감수할 때만")
    p.add_argument("--dry-run", action="store_true")
    p.set_defaults(fn=cmd_publish)

    a = ap.parse_args()
    if a.self_test:
        return _run_self_test()
    if not getattr(a, "fn", None):
        ap.print_help()
        return 2
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
