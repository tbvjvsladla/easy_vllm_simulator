#!/usr/bin/env python3
"""layer_ledger.py -- 분류표 원장과 공통층 동기 지점 (policy BRANCH_CONSTITUTION_LAYERING)

사용법:
  layer_ledger.py common-layer  --repo .                       공통층 pathspec 전수(동기화 대상)
  layer_ledger.py sync-point    --repo . --branch <Z>          Z 의 마지막 공통층 동기 지점 커밋
  layer_ledger.py divergence    --repo . --branch <Z>          동기 지점 이후 Z 의 공통층 갈라짐
  layer_ledger.py append        --repo . --entry-file <json>   분류표 항목 추가(append-only)
  layer_ledger.py merge         --repo . --from-ref <ref>      반대 브랜치 원장과 entry_id 합집합
  layer_ledger.py --self-test

왜 원장인가
-----------
"작업 중 헌법 갱신은 우선 공통층에 들어간다" 가 안전하려면, **어느 갱신이 어느 판정을 거쳤는지**가
파일에 남아야 한다. 대화 기억이 그것을 나르면 세션이 끊기는 순간 사라지고, 다음 사람은 두 브랜치가
왜 이렇게 갈라졌는지 알 수 없다. 캠페인 체인이 같은 병을 앓았고 처방도 같았다 -- 규율이 아니라 **거처**.

원장은 **양 브랜치가 각자 쌓는다.** 그래서 동기화는 이 파일을 복사하지 않고(복사는 반대편 이력을
지운다) `merge` 가 entry_id 합집합으로 수렴시킨다.

동기 지점을 해시로 적지 않는 이유
---------------------------------
한 항목은 *출발* 브랜치와 그 커밋만 적는다. 대상 브랜치에서의 대응 커밋은 **탐색으로 찾는다** --
"항목 시각 이후 Z 의 커밋 중, 공통층 트리가 출발 커밋의 것과 같아지는 최초 커밋". 두 자리에 해시를
적어 두면 한쪽이 rebase·amend 될 때 조용히 거짓이 되고, 커밋 메시지로 찾으면 이름을 바꾸는 순간
술어가 눈이 먼다. 트리 동일성은 이름과 무관하다.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_RED = 5

LEDGER_REL = ".claude/policies/branch_layer_ledger.json"
SYNC_SCRIPT_REL = ".claude/skills/upstream-version-watch/scripts/sync_branches.sh"
SCHEMA_VERSION = 1

VERDICTS = ("common_promote", "branch_only")

#: 동기화 스크립트의 bash 배열을 **읽는다**(다시 적지 않는다). 같은 목록이 두 자리에 있으면
#: 갈라지고, 갈라진 쪽이 조용히 늦는다 -- 이 저장소가 그 형태로 네 번 침묵 누락을 냈다.
#: 배포검증이 이미 같은 방식으로 이 스크립트의 배열을 파싱하고 있어 선례도 있다.
_SH_ARRAY_RE = r"^{name}=\((.*?)^\)"


def _git(repo: Path, *args: str, check: bool = False) -> subprocess.CompletedProcess:
    proc = subprocess.run(["git", "-C", str(repo), *args],
                          capture_output=True, text=True, timeout=120)
    if check and proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {proc.stderr.strip()[:300]}")
    return proc


def _shell_array(src: str, name: str) -> list[str]:
    m = re.search(_SH_ARRAY_RE.format(name=re.escape(name)), src, re.S | re.M)
    if not m:
        return []
    out = []
    for line in m.group(1).splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.append(line)
    return out


def _exclude_pathspecs(src: str) -> list[str]:
    """`PATHS+=(':(exclude)…')` 로 선언된 제외 전수. 순서는 선언 순."""
    return re.findall(r"PATHS\+=\((.*?)\)", src, re.S) and [
        tok for tok in re.findall(r"':\(exclude\)[^']*'", src)
    ] or []


def common_layer(repo: Path) -> dict:
    """동기화 대상(공통층) pathspec. 소유자는 `sync_branches.sh` 의 배열이고 여기서는 읽기만 한다."""
    src = (repo / SYNC_SCRIPT_REL).read_text(encoding="utf-8")
    include = _shell_array(src, "ALLOWLIST")
    # docs 평면은 배열이 아니라 case 패턴으로 선언돼 있다 -- 그 세 패턴을 그대로 읽는다.
    docs = re.search(r"docs/report/\*\)|docs/\*/example\.md\|docs/benchmark/benchmark_\*\.yaml\)", src)
    include_docs = ["docs/report", "docs/benchmark"] if docs else []
    excludes = [tok.strip("'") for tok in _exclude_pathspecs(src)]
    return {"include": include, "include_docs": include_docs, "exclude": excludes,
            "source": SYNC_SCRIPT_REL}


def _diff_pathspec(repo: Path) -> list[str]:
    cl = common_layer(repo)
    # docs/report 는 합집합 수렴 대상이라 **갈라짐 판정에서 뺀다** -- 양쪽에 다른 발행본이 있는
    # 것이 정상이고, 같은 경로의 충돌만 동기화가 따로 RED 로 잡는다.
    return cl["include"] + cl["exclude"] + [":(exclude)docs/report"]


def sync_point(repo: Path, branch: str) -> dict:
    """브랜치 `branch` 의 마지막 공통층 동기 지점 커밋.

    원장의 마지막 항목이 이 브랜치에서 출발했으면 그 커밋이 곧 동기 지점이다. 반대 브랜치에서
    출발했으면, 그 항목 이후 이 브랜치의 커밋 중 **공통층 트리가 출발 커밋의 것과 같아지는 최초
    커밋**을 찾는다(= 동기화가 내려앉은 지점).
    """
    entries = load(repo).get("entries") or []
    if not entries:
        return {"status": "empty-ledger", "commit": None,
                "detail": "원장에 항목이 없다 -- 첫 실행(--bootstrap)이다"}
    last = entries[-1]
    if last.get("departure") == branch:
        return {"status": "departure", "commit": last.get("departure_commit"),
                "entry_id": last.get("entry_id")}

    dep_commit = last.get("departure_commit")
    if not dep_commit:
        return {"status": "unresolved", "commit": None,
                "detail": "마지막 항목에 출발 커밋이 없다"}
    spec = _diff_pathspec(repo)
    log = _git(repo, "rev-list", "--reverse", f"{dep_commit}..{branch}" if False else branch,
               "--max-count=200")
    for sha in log.stdout.split():
        same = _git(repo, "diff", "--quiet", dep_commit, sha, "--", *spec)
        if same.returncode == 0:
            return {"status": "converged", "commit": sha, "entry_id": last.get("entry_id")}
    return {"status": "unresolved", "commit": None, "entry_id": last.get("entry_id"),
            "detail": ("출발 커밋과 공통층 트리가 같아지는 커밋을 이 브랜치에서 찾지 못했다 -- "
                       "중단된 동기화이거나 이 브랜치가 그 이후 공통층을 고쳤다")}


def divergence(repo: Path, branch: str, since: str | None = None) -> dict:
    """동기 지점 이후 `branch` 의 공통층이 갈라졌는가. 갈라졌으면 파일과 커밋 목록을 함께 준다."""
    base = since or (sync_point(repo, branch).get("commit"))
    if not base:
        return {"status": "no-sync-point", "diverged": None,
                "detail": "동기 지점이 없어 갈라짐을 판정할 수 없다(첫 실행이거나 중단된 동기화)"}
    spec = _diff_pathspec(repo)
    quiet = _git(repo, "diff", "--quiet", base, branch, "--", *spec)
    if quiet.returncode == 0:
        return {"status": "clean", "diverged": False, "base": base, "files": [], "commits": []}
    names = _git(repo, "diff", "--name-only", "-z", base, branch, "--", *spec)
    files = [f for f in names.stdout.split("\0") if f]
    commits = _git(repo, "log", "--oneline", f"{base}..{branch}", "--", *spec).stdout.splitlines()
    return {"status": "diverged", "diverged": True, "base": base,
            "files": files, "commits": commits,
            "remediation": ("반대 브랜치의 공통층이 출발 브랜치에 없는 변경을 들고 있다. 덮어쓰면 "
                            "사라진다 -- 위 커밋을 출발 브랜치로 먼저 옮긴 뒤(cherry-pick) 다시 동기화하라.")}


def load(repo: Path) -> dict:
    path = repo / LEDGER_REL
    if not path.is_file():
        return {"schema_version": SCHEMA_VERSION, "entries": []}
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {"schema_version": SCHEMA_VERSION, "entries": []}
    return doc if isinstance(doc, dict) else {"schema_version": SCHEMA_VERSION, "entries": []}


def save(repo: Path, doc: dict) -> Path:
    path = repo / LEDGER_REL
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def validate_entry(entry: dict) -> list[str]:
    problems = []
    for key in ("entry_id", "utc", "departure", "departure_commit", "approved_by",
                "approved_utc", "classification"):
        if not entry.get(key):
            problems.append(f"필수 필드 없음: {key}")
    for row in entry.get("classification") or []:
        if not isinstance(row, dict):
            problems.append(f"분류 행이 객체가 아니다: {row!r}")
            continue
        if not row.get("file"):
            problems.append("분류 행에 file 이 없다")
        if row.get("verdict") not in VERDICTS:
            problems.append(f"판정은 {VERDICTS} 중 하나여야 한다: {row.get('verdict')!r}")
        if not row.get("reason"):
            problems.append(f"분류 행에 근거가 없다: {row.get('file')!r} -- 근거 없는 판정은 "
                            "사람이 검토할 수 없다")
        if row.get("provenance") != "agent-judged":
            problems.append(f"provenance 는 'agent-judged' 여야 한다: {row.get('provenance')!r}")
    return problems


def append(repo: Path, entry: dict) -> dict:
    problems = validate_entry(entry)
    if problems:
        return {"status": "rejected", "problems": problems}
    doc = load(repo)
    existing = {e.get("entry_id") for e in doc.get("entries") or []}
    if entry["entry_id"] in existing:
        return {"status": "rejected",
                "problems": [f"entry_id 중복: {entry['entry_id']} -- 원장은 append-only 다"]}
    doc.setdefault("entries", []).append(entry)
    doc["schema_version"] = SCHEMA_VERSION
    save(repo, doc)
    return {"status": "appended", "entry_id": entry["entry_id"],
            "total": len(doc["entries"]), "path": LEDGER_REL}


def merge(repo: Path, from_ref: str) -> dict:
    """반대 브랜치 원장과 entry_id 합집합. 같은 id 는 **기존 것을 이긴다**(append-only 보존)."""
    proc = _git(repo, "show", f"{from_ref}:{LEDGER_REL}")
    if proc.returncode != 0:
        return {"status": "absent", "detail": f"{from_ref} 에 원장이 없다", "added": 0}
    try:
        other = json.loads(proc.stdout)
    except ValueError as exc:
        return {"status": "unreadable", "detail": str(exc), "added": 0}
    doc = load(repo)
    have = {e.get("entry_id") for e in doc.get("entries") or []}
    added = [e for e in (other.get("entries") or []) if e.get("entry_id") not in have]
    if added:
        doc.setdefault("entries", []).extend(added)
        doc["entries"].sort(key=lambda e: (e.get("utc") or "", e.get("entry_id") or ""))
        doc["schema_version"] = SCHEMA_VERSION
        save(repo, doc)
    return {"status": "merged", "added": len(added),
            "added_ids": [e.get("entry_id") for e in added], "total": len(doc.get("entries") or [])}


def _self_test() -> int:
    import tempfile

    def ck(name: str, cond: bool, got: object = "") -> None:
        if not cond:
            raise RuntimeError(f"[layer_ledger] self-test FAIL: {name} (got={got!r})")

    good = {"entry_id": "e1", "utc": "2026-09-12T00:00:00Z", "departure": "multi-node",
            "departure_commit": "deadbee", "approved_by": "maintainer",
            "approved_utc": "2026-09-12T00:01:00Z",
            "classification": [{"file": "CLAUDE.md", "verdict": "common_promote",
                                "reason": "양 토폴로지가 읽는다", "provenance": "agent-judged"}]}
    ck("양성: 완전한 항목", validate_entry(good) == [], validate_entry(good))
    for mutate, why in (
        ({"classification": [dict(good["classification"][0], reason="")]}, "근거 없음"),
        ({"classification": [dict(good["classification"][0], verdict="maybe")]}, "판정 값역"),
        ({"classification": [dict(good["classification"][0], provenance="human")]}, "provenance"),
        ({"approved_by": ""}, "승인자 없음"),
    ):
        ck(f"음성: {why}", validate_entry(dict(good, **mutate)) != [], mutate)

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        (root / ".claude/policies").mkdir(parents=True)
        ck("첫 append", append(root, good)["status"] == "appended")
        ck("중복 거부(append-only)", append(root, good)["status"] == "rejected")
        ck("적재", len(load(root)["entries"]) == 1)

    print("[layer_ledger] self-test PASS", file=sys.stderr)
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    for name in ("common-layer", "sync-point", "divergence", "append", "merge"):
        sp = sub.add_parser(name)
        sp.add_argument("--repo", default=".")
        if name in ("sync-point", "divergence"):
            sp.add_argument("--branch", required=True)
        if name == "divergence":
            sp.add_argument("--since", default=None)
        if name == "append":
            sp.add_argument("--entry-file", required=True)
        if name == "merge":
            sp.add_argument("--from-ref", required=True)
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.cmd:
        ap.print_usage(sys.stderr)
        return EXIT_USAGE

    repo = Path(args.repo).resolve()
    if args.cmd == "common-layer":
        res = common_layer(repo)
    elif args.cmd == "sync-point":
        res = sync_point(repo, args.branch)
    elif args.cmd == "divergence":
        res = divergence(repo, args.branch, args.since)
    elif args.cmd == "append":
        res = append(repo, json.loads(Path(args.entry_file).read_text(encoding="utf-8")))
    else:
        res = merge(repo, args.from_ref)

    print(json.dumps(res, ensure_ascii=False, indent=2))
    if args.cmd == "divergence" and res.get("diverged"):
        return EXIT_RED
    if args.cmd == "append" and res.get("status") == "rejected":
        return EXIT_RED
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
