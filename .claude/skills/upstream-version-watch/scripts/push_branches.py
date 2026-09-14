#!/usr/bin/env python3
"""push_branches.py -- 종료 시퀀스의 원격 반영. PAT 경로는 hint-publisher 가 소유한다.

사용법:
  push_branches.py --remote <원격> --branch <B> [--branch <B2> ...] [--dry-run] [--repo .]
  push_branches.py --self-test

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

결과는 **항상** 브랜치별 JSON 이다 (2026-09-14 · ⑧-pre D2 S5)
-------------------------------------------------------------
`hint_tag` 는 실패를 `die()` = `SystemExit` 로 알린다(자격증명 부재 · `git push` 거부). `SystemExit` 은
`Exception` 이 아니라서 종전 `except Exception` 을 그대로 뚫었고, 이 스크립트는 **JSON 없이 rc 1** 로 끝나며
남은 브랜치를 시도하지도 않았다 -- 호출부는 "어느 브랜치가 왜" 를 알 수 없었다. 이제 브랜치마다 그 사유
(die 가 stderr 로 낸 문장)를 `stderr_tail` 에 담고 다음 브랜치로 간다. 강제 push 는 여전히 없다(refspec 에 `+` 없음).

같은 이유로 적재도 **대상 저장소를 기준으로** 한다. `hint_tag.ROOT` 는 import 시점의 CWD 로 정해지므로
`--repo` 와 다른 곳에서 부르면 엉뚱한 저장소를 밀거나(git 저장소 안) 적재 자체가 `SystemExit` 으로 죽었다(밖).
"""
from __future__ import annotations

import argparse
import contextlib
import importlib.util
import io
import json
import os
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_PUSH_FAILED = 5

HINT_TAG_REL = ".claude/skills/hint-publisher/scripts/hint_tag.py"
OPERATIONAL_BRANCHES = ("single-node", "multi-node")


def _load_hint_tag(repo: Path) -> tuple[object | None, str]:
    """(모듈 또는 None, 사유). 적재는 `repo` 를 CWD 로 두고 한다 -- `hint_tag.ROOT` 가 import 시점 CWD 에서 정해진다."""
    path = repo / HINT_TAG_REL
    if not path.is_file():
        return None, f"{HINT_TAG_REL} 부재"
    # 파일 경로 적재이지만 **디렉터리를 sys.path 에 먼저 넣는다** -- hint_tag 의 sibling import 가
    # 그것 없이는 ModuleNotFoundError 로 죽는다(실측).
    scripts_dir = str(path.parent)
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)
    spec = importlib.util.spec_from_file_location("_push_branches_hint_tag", path)
    if spec is None or spec.loader is None:
        return None, "적재 사양을 만들 수 없다"
    module = importlib.util.module_from_spec(spec)
    cwd = os.getcwd()
    buf = io.StringIO()
    try:
        os.chdir(repo)
        with contextlib.redirect_stderr(buf):
            spec.loader.exec_module(module)
    except (Exception, SystemExit) as exc:  # noqa: BLE001 -- 적재 실패는 사유와 함께 보고한다(die 는 SystemExit)
        return None, f"적재 실패({type(exc).__name__}): {(buf.getvalue() or str(exc)).strip()[-300:]}"
    finally:
        os.chdir(cwd)
    root = Path(getattr(module, "ROOT", "")).resolve()
    if root != repo.resolve():
        return None, f"hint_tag.ROOT={root} 가 대상 저장소 {repo.resolve()} 와 다르다"
    return module, "ok"


def push(repo: Path, remote: str, branches: list[str], *, dry_run: bool) -> dict:
    results = []
    mod, load_note = _load_hint_tag(repo)
    for br in branches:
        refspec = f"refs/heads/{br}:refs/heads/{br}"
        used = "hint_tag.git_push_authenticated"
        buf = io.StringIO()
        try:
            if mod is not None and hasattr(mod, "git_push_authenticated"):
                with contextlib.redirect_stderr(buf):
                    proc = mod.git_push_authenticated(remote, refspec, dry_run=dry_run)
                rc = proc.returncode
                err = (proc.stderr or "")[-400:]
            else:
                used = f"git push (fallback -- hint_tag 미적재: {load_note})"
                args = ["git", "-C", str(repo), "push"]
                if dry_run:
                    args.append("--dry-run")
                args += [remote, refspec]
                proc = subprocess.run(args, capture_output=True, text=True, timeout=300)
                rc = proc.returncode
                err = (proc.stderr or "")[-400:]
        except SystemExit as exc:
            # hint_tag.die -- 사유 문장은 die 가 stderr 로 냈고 위 버퍼가 잡았다. 코드가 0/None 이어도 실패다.
            rc = exc.code if isinstance(exc.code, int) and exc.code != 0 else 1
            err = (buf.getvalue() or str(exc.code or ""))[-400:]
            used += " → die(SystemExit)"
        except Exception as exc:  # noqa: BLE001
            rc, err = 1, f"{type(exc).__name__}: {exc}"
        finally:
            if buf.getvalue():
                sys.stderr.write(buf.getvalue())       # 사람 화면에도 그대로 남긴다
        results.append({"branch": br, "refspec": refspec, "returncode": rc,
                        "path": used, "stderr_tail": err.strip()})
    ok = all(r["returncode"] == 0 for r in results)
    return {"remote": remote, "dry_run": dry_run, "ok": ok, "results": results}


def _self_test() -> int:
    """격리 저장소 + 임시 bare 원격에서 **실제 hint_tag 사본**으로 실패 경로를 친다(원격·실 토큰 무관).

    `assert` 를 쓰지 않는다 -- `-O` 에서 사라지는 검사는 검사가 아니다(runtime_selftest 가 전수 금지).
    """
    import shutil
    import tempfile

    here = Path(__file__).resolve()
    real_hint_tag = here.parents[2] / "hint-publisher" / "scripts" / "hint_tag.py"
    if not real_hint_tag.is_file():
        print(f"[push_branches] self-test FAIL: 실물 hint_tag 가 없다: {real_hint_tag}", file=sys.stderr)
        return 1
    failures: list[str] = []

    def ck(name: str, cond: bool, got: object = "") -> None:
        print(f"  {'ok  ' if cond else 'FAIL'} {name}", file=sys.stderr)
        if not cond:
            failures.append(f"{name} (got={got!r})"[:600])

    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        gcfg = tdp / "gitconfig"
        gcfg.write_text("[user]\n\tname = selftest\n\temail = t@t\n[commit]\n\tgpgsign = false\n"
                        "[core]\n\thooksPath = /dev/null\n", encoding="utf-8")
        env = {k: v for k, v in os.environ.items()
               if k not in ("GITHUB_TOKEN", "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                            "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX")}
        env.update(GIT_CONFIG_GLOBAL=str(gcfg), GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0")

        def g(*args: str, cwd: Path) -> str:
            proc = subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True, env=env, timeout=60)
            if proc.returncode != 0:
                raise RuntimeError(f"fixture git {args} failed: {proc.stderr.strip()[:300]}")
            return proc.stdout.strip()

        remote = tdp / "remote.git"
        g("init", "-q", "--bare", str(remote), cwd=tdp)
        repo = tdp / "repo"
        repo.mkdir()
        g("init", "-q", "-b", "multi-node", cwd=repo)
        (repo / HINT_TAG_REL).parent.mkdir(parents=True)
        shutil.copy2(real_hint_tag, repo / HINT_TAG_REL)
        (repo / "f.txt").write_text("1\n", encoding="utf-8")
        g("add", "-A", cwd=repo)
        g("commit", "-qm", "base", cwd=repo)
        g("branch", "single-node", cwd=repo)
        g("push", "-q", str(remote), "refs/heads/multi-node:refs/heads/multi-node",
          "refs/heads/single-node:refs/heads/single-node", cwd=repo)
        # multi-node: 앞으로 감을 수 있는 커밋 · single-node: 원격이 다른 커밋을 들고 있어 non-fast-forward
        (repo / "f.txt").write_text("2\n", encoding="utf-8")
        g("commit", "-qam", "multi ahead", cwd=repo)
        g("checkout", "-q", "single-node", cwd=repo)
        (repo / "g.txt").write_text("remote-only\n", encoding="utf-8")
        g("add", "-A", cwd=repo)
        g("commit", "-qm", "remote-only", cwd=repo)
        g("push", "-q", str(remote), "refs/heads/single-node:refs/heads/single-node", cwd=repo)
        g("reset", "-q", "--hard", "HEAD~1", cwd=repo)
        (repo / "h.txt").write_text("local-diverged\n", encoding="utf-8")
        g("add", "-A", cwd=repo)
        g("commit", "-qm", "local diverged", cwd=repo)
        g("checkout", "-q", "multi-node", cwd=repo)
        remote_single_before = g("rev-parse", "refs/heads/single-node", cwd=remote)
        remote_multi_before = g("rev-parse", "refs/heads/multi-node", cwd=remote)
        local_multi = g("rev-parse", "multi-node", cwd=repo)

        # 다른 git 저장소를 CWD 로 둔다 -- 대상 저장소 기준 적재가 아니면 이 저장소를 밀게 된다.
        decoy = tdp / "decoy"
        decoy.mkdir()
        g("init", "-q", "-b", "multi-node", cwd=decoy)
        (decoy / "d.txt").write_text("decoy\n", encoding="utf-8")
        g("add", "-A", cwd=decoy)
        g("commit", "-qm", "decoy", cwd=decoy)

        def run(script: Path, extra_env: dict, cwd: Path) -> tuple[int, dict | None, str]:
            proc = subprocess.run([sys.executable, str(script), "--repo", str(repo), "--remote", str(remote),
                                   "--branch", "single-node", "--branch", "multi-node"],
                                  capture_output=True, text=True, env={**env, **extra_env}, cwd=str(cwd), timeout=120)
            try:
                doc = json.loads(proc.stdout)
            except ValueError:
                doc = None
            return proc.returncode, doc, proc.stderr

        # A: 자격증명 부재 → hint_tag.die → 두 브랜치 모두 사유와 함께 JSON 으로
        rc, doc, _ = run(here, {}, tdp)
        ck("★S5 자격증명 부재(die) → JSON 결과 · rc=5", rc == EXIT_PUSH_FAILED and isinstance(doc, dict), (rc, doc))
        res = (doc or {}).get("results") or []
        ck("★S5 브랜치 두 개 모두 시도·보고(첫 실패에서 멈추지 않는다)",
           [r.get("branch") for r in res] == ["single-node", "multi-node"], res)
        ck("★S5 실패 사유 = die 문장(stderr_tail) · rc≠0 · 경로 표시",
           all(r.get("returncode") != 0 and "자격증명" in (r.get("stderr_tail") or "")
               and "die(SystemExit)" in (r.get("path") or "") for r in res), res)
        ck("자격증명 부재에서는 원격이 그대로다",
           g("rev-parse", "refs/heads/multi-node", cwd=remote) == remote_multi_before
           and g("rev-parse", "refs/heads/single-node", cwd=remote) == remote_single_before)

        # B: 토큰(자리값) 있음 · single-node non-ff 거부(die) 뒤에도 multi-node 는 밀린다 · 강제 없음
        rc, doc, _ = run(here, {"GITHUB_TOKEN": "selftest-placeholder"}, decoy)
        res = (doc or {}).get("results") or []
        by = {r.get("branch"): r for r in res}
        ck("★S5 non-fast-forward 거부 → 그 브랜치만 실패 · 다음 브랜치는 시도해 성공 · rc=5",
           rc == EXIT_PUSH_FAILED and by.get("single-node", {}).get("returncode") not in (0, None)
           and by.get("multi-node", {}).get("returncode") == 0, (rc, res))
        ck("★강제 push 없음: 원격 single-node 는 그대로",
           g("rev-parse", "refs/heads/single-node", cwd=remote) == remote_single_before)
        ck("★대상 저장소 기준 적재: CWD 가 다른 저장소여도 --repo 의 커밋을 민다",
           g("rev-parse", "refs/heads/multi-node", cwd=remote) == local_multi)

        # 변이 음성대조: `except SystemExit` 를 걷어낸 사본은 JSON 없이 죽는다 -- 이 검사가 그 회귀를 잡는다
        mutated = tdp / "push_branches_mutated.py"
        src = here.read_text(encoding="utf-8")
        needle = "        except SystemExit as exc:\n"
        if src.count(needle) != 1:
            ck("변이 앵커(except SystemExit) 1건", False, src.count(needle))
        else:
            mutated.write_text(src.replace(needle, "        except KeyboardInterrupt as exc:\n"), encoding="utf-8")
            rc_m, doc_m, _ = run(mutated, {}, tdp)
            ck("★변이 음성대조: SystemExit 을 안 잡으면 JSON 이 없다(결함 재현)", doc_m is None and rc_m != 0, (rc_m, doc_m))

    if failures:
        for f in failures:
            print(f"[push_branches] self-test FAIL: {f}", file=sys.stderr)
        return 1
    print("[push_branches] self-test PASS", file=sys.stderr)
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--remote")
    ap.add_argument("--branch", action="append", default=[],
                    help="반복 지정 가능. 선언된 브랜치만 민다(--tags 금지 · hint 태그는 별도 평면)")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.remote or not args.branch:
        print("[push_branches] FAIL: --remote 와 --branch 가 필요하다", file=sys.stderr)
        return EXIT_USAGE

    for br in args.branch:
        if br not in OPERATIONAL_BRANCHES:
            print(f"[push_branches] FAIL: 운영 refs 는 single-node·multi-node 뿐이다: {br!r}",
                  file=sys.stderr)
            return EXIT_USAGE

    res = push(Path(args.repo).resolve(), args.remote, args.branch, dry_run=args.dry_run)
    print(json.dumps(res, ensure_ascii=False, indent=2))
    return EXIT_OK if res["ok"] else EXIT_PUSH_FAILED


if __name__ == "__main__":
    sys.exit(main())
