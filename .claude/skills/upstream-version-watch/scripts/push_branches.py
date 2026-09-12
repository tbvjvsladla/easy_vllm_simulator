#!/usr/bin/env python3
"""push_branches.py -- 종료 시퀀스의 원격 반영. PAT 경로는 hint-publisher 가 소유한다.

사용법:
  push_branches.py --remote <원격> --branch <B> [--branch <B2> ...] [--dry-run] [--repo .]

왜 여기서 자격증명을 다시 구현하지 않나
---------------------------------------
무인 push 의 자격증명 경로(`GITHUB_TOKEN` 조회 → `credential.helper` 를 `-c` 인자로만 전달 →
프로세스 인자표·reflog·remote.url 에 토큰을 남기지 않음)는 이미 `hint_tag.py` 가 갖고 있고, 그
배선이 없던 시절 무인 실행이 `could not read Username` 으로 죽은 실증이 그 파일 주석에 남아 있다.
같은 규칙을 두 번째 자리에 적으면 갈라지고, 갈라진 쪽이 조용히 늦는다.

그래서 이 스크립트는 **소비자**다 -- 함수를 옮겨 오지 않고 그 모듈을 적재해 부른다. 옮겨 오면
`hint_tag --self-test` 의 자격증명 검사 10건이 모듈 네임스페이스에서 이름을 찾지 못해 깨지고,
파일 경로로 hint_tag 를 적재하는 로더 셋이 `sys.path` 를 세우지 않아 sibling import 도 깨진다.

SSH 원격에서는 helper 가 조회되지 않으므로 이 경로는 https 원격에서만 실질적으로 작동한다 --
그것이 결함은 아니다. SSH 는 에이전트가 이미 인증을 갖고 있다는 뜻이다.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PUSH_FAILED = 5

HINT_TAG_REL = ".claude/skills/hint-publisher/scripts/hint_tag.py"


def _load_hint_tag(repo: Path):
    path = repo / HINT_TAG_REL
    if not path.is_file():
        return None
    # 파일 경로 적재이지만 **디렉터리를 sys.path 에 먼저 넣는다** -- hint_tag 의 sibling import 가
    # 그것 없이는 ModuleNotFoundError 로 죽는다(실측).
    scripts_dir = str(path.parent)
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)
    spec = importlib.util.spec_from_file_location("_push_branches_hint_tag", path)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
    except Exception:  # noqa: BLE001
        return None
    return module


def push(repo: Path, remote: str, branches: list[str], *, dry_run: bool) -> dict:
    results = []
    mod = _load_hint_tag(repo)
    for br in branches:
        refspec = f"refs/heads/{br}:refs/heads/{br}"
        used = "hint_tag.git_push_authenticated"
        try:
            if mod is not None and hasattr(mod, "git_push_authenticated"):
                proc = mod.git_push_authenticated(remote, refspec, dry_run=dry_run)
                rc = proc.returncode
                err = (proc.stderr or "")[-400:]
            else:
                used = "git push (fallback -- hint_tag 미적재)"
                args = ["git", "-C", str(repo), "push"]
                if dry_run:
                    args.append("--dry-run")
                args += [remote, refspec]
                proc = subprocess.run(args, capture_output=True, text=True, timeout=300)
                rc = proc.returncode
                err = (proc.stderr or "")[-400:]
        except Exception as exc:  # noqa: BLE001
            rc, err = 1, f"{type(exc).__name__}: {exc}"
        results.append({"branch": br, "refspec": refspec, "returncode": rc,
                        "path": used, "stderr_tail": err.strip()})
    ok = all(r["returncode"] == 0 for r in results)
    return {"remote": remote, "dry_run": dry_run, "ok": ok, "results": results}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=".")
    ap.add_argument("--remote", required=True)
    ap.add_argument("--branch", action="append", required=True,
                    help="반복 지정 가능. 선언된 브랜치만 민다(--tags 금지 · hint 태그는 별도 평면)")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args(argv)

    for br in args.branch:
        if br not in ("single-node", "multi-node"):
            print(f"[push_branches] FAIL: 운영 refs 는 single-node·multi-node 뿐이다: {br!r}",
                  file=sys.stderr)
            return EXIT_USAGE

    res = push(Path(args.repo).resolve(), args.remote, args.branch, dry_run=args.dry_run)
    print(json.dumps(res, ensure_ascii=False, indent=2))
    return EXIT_OK if res["ok"] else EXIT_PUSH_FAILED


if __name__ == "__main__":
    sys.exit(main())
