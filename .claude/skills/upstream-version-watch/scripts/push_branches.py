#!/usr/bin/env python3
"""push_branches.py -- 종료 시퀀스의 원격 반영. PAT 경로는 hint-publisher(`hintlib/tag.py`)가 소유한다.

사용법:
  push_branches.py --remote <원격> --branch <B> [--branch <B2> ...] [--dry-run] [--repo .]
  push_branches.py --self-test

왜 여기서 자격증명을 다시 구현하지 않나
---------------------------------------
무인 push 의 자격증명 경로(스킴 판정 → https 면 `GITHUB_TOKEN` 조회 → `credential.helper` 를 `-c` 인자로만 전달 →
프로세스 인자표·reflog·remote.url 에 토큰을 남기지 않음 · `GIT_TERMINAL_PROMPT=0` · `--no-follow-tags`)는 hint-publisher 의
공개 API `hintlib.tag.git_push_authenticated(repo, remote, refspec, dry_run=)` 가 갖고 있고, 그 배선이 없던 시절 무인
실행이 `could not read Username` 으로 죽은 실증(2026-09-04)이 그 모듈 docstring 에 남아 있다. 같은 규칙을 두 번째 자리에
적으면 갈라지고, 갈라진 쪽이 조용히 늦는다.

그래서 이 스크립트는 **소비자**다 -- 함수를 옮겨 오지 않고 그 패키지를 적재해 부른다. 자격증명 규칙 자체의 검사
(토큰 argv 부재 · 상속 helper 비움 · dry-run 실패 삼킴 ✗ · followTags 차단 · 평문 http 거부)는 `hintlib/tag.py` 자체검사가
소유하고, 여기 자체검사는 **소비 배선**(적재 위치 · 실패 분류 · 브랜치별 결과 · 강제 ✗)만 친다.

스킴 인식 (2026-09-21 · plan_26092119 §4.8 · 코드맵 H7)
    토큰은 **https 원격에만** 필요하다. ssh·scp형·로컬 경로는 토큰 없이 민다. 옛 경로는 SSH 여도 `GITHUB_TOKEN` 이
    없으면 죽었다 -- 종료 시퀀스의 기본 원격(`github-ssh`)에서 PAT 를 요구하던 잠복 결함이다. 판정은 **선언된**
    URL(insteadOf 전개 전)로 한다(전개 후로 판정하면 insteadOf 한 줄이 자격증명 요구를 조용히 끈다).

결과는 **항상** 브랜치별 JSON 이다 (2026-09-14 · ⑧-pre D2 S5)
-------------------------------------------------------------
옛 `hint_tag` 는 실패를 `die()` = `SystemExit` 로 알렸다(자격증명 부재 · `git push` 거부). `SystemExit` 은
`Exception` 이 아니라서 종전 `except Exception` 을 그대로 뚫었고, 이 스크립트는 **JSON 없이 rc 1** 로 끝나며
남은 브랜치를 시도하지도 않았다 -- 호출부는 "어느 브랜치가 왜" 를 알 수 없었다. 이제 브랜치마다 그 사유를
`stderr_tail` 에 담고 다음 브랜치로 간다. 강제 push 는 여전히 없다(refspec 에 `+` 없음 · 공개 API 도 `+` 를 거부한다).
2026-09-22: hintlib 은 실패를 `core.HintError(code, message, remedy)`(`Exception` 계열)로 알린다. 브랜치 결과에
**`code`**(사유코드)를 싣는다 -- 판정은 code 로만 한다(메시지 substring ✗ · hintlib 규약). 자체검사의 변이 음성대조는
HintError 처리 절을 걷어낸 사본에서 이 분류가 사라지는지를 본다.

적재는 **대상 저장소를 기준으로** 한다
    옛 `hint_tag.ROOT` 는 import 시점의 CWD 로 정해져서 `--repo` 와 다른 곳에서 부르면 엉뚱한 저장소를 밀거나(git
    저장소 안) 적재 자체가 `SystemExit` 으로 죽었다(밖). hintlib 은 import 부수효과가 0 이고 저장소를 인자로 받는다 --
    그래도 **`--repo` 의 hintlib 판본**을 고유 패키지 이름으로 적재하고, 적재된 파일이 그 저장소 아래인지 확인한다
    (push 하는 저장소와 규칙의 판본이 같게 · CWD 에 무엇이 있든 무관하게).

폴백 (fail-loud · 2026-09-22 강화)
    대상 저장소의 hintlib 을 적재할 수 없으면 맨 `git push` 로 민다 -- 결과 `path` 에 폴백과 그 사유를 적고 stderr
    에도 경고한다(침묵 폴백 ✗). 이 경로는 자격증명 helper 를 싣지 않으므로(https 면 인증 실패로 끝난다)
    `GIT_TERMINAL_PROMPT=0` 으로 대기 대신 즉시 실패하고, `--no-follow-tags --no-recurse-submodules` 를 붙인다 --
    운영자 설정 `push.followTags=true` 가 브랜치 push 에 로컬 `last-good-*` 태그를 얹는 경로(2026-09-22 hintlib 실측:
    태그 1개 push 가 3개를 올렸다)를 폴백도 막는다. 이 옵션 목록은 `hintlib.tag.PUSH_SCOPE_ARGS` 와 같아야 하며
    자체검사가 교차 대조한다(적재 실패 시에도 필요한 값이라 단일 소유가 불가능한 자리 · 교차검증이 차선).
"""
from __future__ import annotations

import argparse
import contextlib
import importlib
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

HINTLIB_REL = ".claude/skills/hint-publisher/scripts/hintlib"
# 고유 패키지 이름 -- 같은 프로세스의 다른 `hintlib` 적재(다른 저장소 판본)와 sys.modules 에서 섞이지 않게.
_HINTLIB_PKG = "_push_branches_hintlib"
OPERATIONAL_BRANCHES = ("single-node", "multi-node")
# 폴백 push 의 범위 옵션 -- `hintlib.tag.PUSH_SCOPE_ARGS` 와 같아야 한다(자체검사가 교차 대조 · 모듈 docstring).
FALLBACK_PUSH_SCOPE_ARGS = ("--no-follow-tags", "--no-recurse-submodules")


class _HintlibAbsent(Exception):
    """hintlib 미적재 시 `except` 절 자리를 채우는 예외 -- 발생하지 않는다(처리 절을 한 벌로 유지)."""


def _drop_pkg() -> None:
    for k in [k for k in sys.modules if k == _HINTLIB_PKG or k.startswith(_HINTLIB_PKG + ".")]:
        del sys.modules[k]


def _load_hintlib(repo: Path) -> tuple[object | None, type, str]:
    """(tag 모듈 또는 None, HintError 클래스, 사유). `repo` 의 hintlib 패키지를 고유 이름으로 적재한다."""
    pkg_dir = repo / HINTLIB_REL
    init = pkg_dir / "__init__.py"
    if not init.is_file() or not (pkg_dir / "tag.py").is_file():
        return None, _HintlibAbsent, f"{HINTLIB_REL}/tag.py 부재"
    _drop_pkg()
    spec = importlib.util.spec_from_file_location(_HINTLIB_PKG, init, submodule_search_locations=[str(pkg_dir)])
    if spec is None or spec.loader is None:
        return None, _HintlibAbsent, "적재 사양을 만들 수 없다"
    pkg = importlib.util.module_from_spec(spec)
    sys.modules[_HINTLIB_PKG] = pkg
    try:
        spec.loader.exec_module(pkg)
        tag = importlib.import_module(f"{_HINTLIB_PKG}.tag")
        core = importlib.import_module(f"{_HINTLIB_PKG}.core")
    except (Exception, SystemExit) as exc:  # noqa: BLE001 -- 적재 실패는 사유와 함께 보고한다(폴백 경로 표시)
        _drop_pkg()
        return None, _HintlibAbsent, f"적재 실패({type(exc).__name__}): {str(exc).strip()[-300:]}"
    loaded = Path(getattr(tag, "__file__", None) or "").resolve()
    if repo.resolve() not in loaded.parents:
        return None, _HintlibAbsent, f"적재된 hintlib.tag({loaded}) 가 대상 저장소 {repo.resolve()} 아래가 아니다"
    herr = getattr(core, "HintError", None)
    if not callable(getattr(tag, "git_push_authenticated", None)) or not (
            isinstance(herr, type) and issubclass(herr, Exception)):
        return None, _HintlibAbsent, "hintlib 공개 API(tag.git_push_authenticated · core.HintError) 부재"
    return tag, herr, "ok"


def push(repo: Path, remote: str, branches: list[str], *, dry_run: bool) -> dict:
    results = []
    tag, hint_error, load_note = _load_hintlib(repo)
    if tag is None:
        print(f"[push_branches] ⚠ hintlib 미적재({load_note}) -- 폴백 git push(자격증명 helper ✗ · 범위 옵션 유지)",
              file=sys.stderr)
    for br in branches:
        refspec = f"refs/heads/{br}:refs/heads/{br}"
        used = "hintlib.tag.git_push_authenticated"
        code = None
        buf = io.StringIO()
        try:
            if tag is not None:
                with contextlib.redirect_stderr(buf):
                    proc = tag.git_push_authenticated(repo, remote, refspec, dry_run=dry_run)
            else:
                used = f"git push (fallback -- hintlib 미적재: {load_note})"
                args = ["git", "-C", str(repo), "push", *FALLBACK_PUSH_SCOPE_ARGS]
                if dry_run:
                    args.append("--dry-run")
                args += [remote, refspec]
                proc = subprocess.run(args, capture_output=True, text=True, timeout=300,
                                      env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
            rc = proc.returncode
            err = (proc.stderr or "")[-400:]
        except hint_error as exc:
            # hintlib 의 실패(자격증명 부재 · push 거부 · 원격 규율 위반). 사유코드로 분류한다(메시지 substring ✗).
            rc = exc.exit_code if isinstance(getattr(exc, "exit_code", None), int) and exc.exit_code != 0 else 1
            code = getattr(exc, "code", None)
            err = str(getattr(exc, "message", exc))[-400:]
            used += f" → HintError({code})"
            render = getattr(exc, "render", None)
            buf.write((render() if callable(render) else str(exc)) + "\n")   # 처방(remedy)까지 사람 화면에
        except Exception as exc:  # noqa: BLE001 -- 예상 밖 실패도 브랜치별 JSON 으로 보고하고 다음 브랜치로 간다
            rc, err = 1, f"{type(exc).__name__}: {exc}"
            used += f" → {type(exc).__name__}"
        finally:
            if buf.getvalue():
                sys.stderr.write(buf.getvalue())       # 사람 화면에도 그대로 남긴다
        results.append({"branch": br, "refspec": refspec, "returncode": rc, "code": code,
                        "path": used, "stderr_tail": err.strip()})
    ok = all(r["returncode"] == 0 for r in results)
    return {"remote": remote, "dry_run": dry_run, "ok": ok, "results": results}


def _self_test() -> int:
    """격리 저장소 + 임시 bare 원격에서 **실제 hintlib 사본**으로 소비 배선을 친다(네트워크·실 토큰 무관).

    https 원격은 예약 TLD(`.invalid`) 주소를 선언하고 insteadOf 로 로컬 bare 에 돌린다 -- 스킴 판정은 선언 URL 로
    하므로 https 규칙(토큰 필수)을 네트워크 없이 칠 수 있다. `assert` 를 쓰지 않는다 -- `-O` 에서 사라지는 검사는
    검사가 아니다(runtime_selftest 가 전수 금지).
    """
    import shutil
    import tempfile

    sys.dont_write_bytecode = True      # 실물 hintlib 교차 대조 적재가 저장소에 __pycache__ 를 남기지 않게
    here = Path(__file__).resolve()
    real_repo = here.parents[4]
    real_hintlib = real_repo / HINTLIB_REL
    if not (real_hintlib / "tag.py").is_file():
        print(f"[push_branches] self-test FAIL: 실물 hintlib 이 없다: {real_hintlib}", file=sys.stderr)
        return 1
    failures: list[str] = []

    def ck(name: str, cond: bool, got: object = "") -> None:
        print(f"  {'ok  ' if cond else 'FAIL'} {name}", file=sys.stderr)
        if not cond:
            failures.append(f"{name} (got={got!r})"[:600])

    # 교차 대조: 폴백 범위 옵션 == hintlib.tag.PUSH_SCOPE_ARGS (단일 소유 불가 자리의 차선)
    real_tag, _, note = _load_hintlib(real_repo)
    ck("폴백 범위 옵션 == hintlib.tag.PUSH_SCOPE_ARGS(교차 대조)",
       real_tag is not None and tuple(getattr(real_tag, "PUSH_SCOPE_ARGS", ())) == FALLBACK_PUSH_SCOPE_ARGS,
       (note, getattr(real_tag, "PUSH_SCOPE_ARGS", None)))
    _drop_pkg()

    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        gcfg = tdp / "gitconfig"
        gcfg.write_text("[user]\n\tname = selftest\n\temail = t@t\n[commit]\n\tgpgsign = false\n"
                        "[tag]\n\tgpgSign = false\n[core]\n\thooksPath = /dev/null\n", encoding="utf-8")
        env = {k: v for k, v in os.environ.items()
               if k not in ("GITHUB_TOKEN", "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                            "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX")}
        env.update(GIT_CONFIG_GLOBAL=str(gcfg), GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0",
                   PYTHONDONTWRITEBYTECODE="1")

        def g(*args: str, cwd: Path) -> str:
            proc = subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True, env=env, timeout=60)
            if proc.returncode != 0:
                raise RuntimeError(f"fixture git {args} failed: {proc.stderr.strip()[:300]}")
            return proc.stdout.strip()

        def bare(name: str) -> Path:
            p = tdp / name
            g("init", "-q", "--bare", str(p), cwd=tdp)
            return p

        def refs(b: Path, prefix: str) -> list[str]:
            return g("for-each-ref", "--format=%(refname)", prefix, cwd=b).splitlines()

        remote = bare("remote.git")
        repo = tdp / "repo"
        repo.mkdir()
        g("init", "-q", "-b", "multi-node", cwd=repo)
        # 실물 hintlib 전체(모듈 전부)를 복사한다 -- 픽스처가 실물보다 좁으면 형제 import 가 실물에서만 깨진다.
        fx_hintlib = repo / HINTLIB_REL
        fx_hintlib.mkdir(parents=True)
        for src in sorted(real_hintlib.glob("*.py")):
            shutil.copy2(src, fx_hintlib / src.name)
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

        # https 로 **선언**한 원격 → insteadOf 로 로컬 bare(예약 TLD · 네트워크 0).
        https_url = "https://fixture.invalid/remote.git"
        g("remote", "add", "fx", https_url, cwd=repo)
        g("config", f"url.{remote}.insteadOf", https_url, cwd=repo)
        # 운영자 설정 push.followTags=true + 밀리는 커밋에 닿는 로컬 롤백 앵커 태그(격리 저장소 픽스처).
        g("config", "push.followTags", "true", cwd=repo)
        g("tag", "-a", "-m", "local rollback anchor", "last-good-fixture", "multi-node", cwd=repo)
        ctrl = bare("ctrl-follow.git")
        g("push", "-q", str(ctrl), "refs/heads/multi-node:refs/heads/multi-node", cwd=repo)
        ck("대조: 옵션 없는 git push 는 followTags 로 태그를 딸려 보낸다(픽스처가 살아 있다)",
           refs(ctrl, "refs/tags") == ["refs/tags/last-good-fixture"], refs(ctrl, "refs/tags"))

        # 다른 git 저장소를 CWD 로 둔다 -- 대상 저장소 기준 적재가 아니면 이 저장소(hintlib 없음)로 폴백하거나 민다.
        decoy = tdp / "decoy"
        decoy.mkdir()
        g("init", "-q", "-b", "multi-node", cwd=decoy)
        (decoy / "d.txt").write_text("decoy\n", encoding="utf-8")
        g("add", "-A", cwd=decoy)
        g("commit", "-qm", "decoy", cwd=decoy)

        def run(script: Path, rmt: str, extra_env: dict, cwd: Path) -> tuple[int, dict | None, str]:
            proc = subprocess.run([sys.executable, str(script), "--repo", str(repo), "--remote", rmt,
                                   "--branch", "single-node", "--branch", "multi-node"],
                                  capture_output=True, text=True, env={**env, **extra_env}, cwd=str(cwd), timeout=180)
            try:
                doc = json.loads(proc.stdout)
            except ValueError:
                doc = None
            return proc.returncode, doc, proc.stderr

        # A: https 선언 원격 · 자격증명 부재 → HintError(HINT_PUSH_CREDENTIAL_ABSENT) → 두 브랜치 모두 사유와 함께 JSON
        rc, doc, _ = run(here, "fx", {}, tdp)
        ck("★S5 자격증명 부재(HintError) → JSON 결과 · rc=5", rc == EXIT_PUSH_FAILED and isinstance(doc, dict), (rc, doc))
        res = (doc or {}).get("results") or []
        ck("★S5 브랜치 두 개 모두 시도·보고(첫 실패에서 멈추지 않는다)",
           [r.get("branch") for r in res] == ["single-node", "multi-node"], res)
        ck("★S5 실패 사유 = 사유코드 HINT_PUSH_CREDENTIAL_ABSENT · 문장(stderr_tail)에 '자격증명' · rc≠0 · 경로 표시",
           len(res) == 2 and all(r.get("returncode") != 0 and r.get("code") == "HINT_PUSH_CREDENTIAL_ABSENT"
                                 and "자격증명" in (r.get("stderr_tail") or "")
                                 and "HintError(HINT_PUSH_CREDENTIAL_ABSENT)" in (r.get("path") or "") for r in res), res)
        ck("자격증명 부재에서는 원격이 그대로다",
           g("rev-parse", "refs/heads/multi-node", cwd=remote) == remote_multi_before
           and g("rev-parse", "refs/heads/single-node", cwd=remote) == remote_single_before)

        # B: 토큰(자리값) 있음 · single-node non-ff 거부 뒤에도 multi-node 는 밀린다 · 강제 없음 · 태그 딸림 없음
        rc, doc, _ = run(here, "fx", {"GITHUB_TOKEN": "selftest-placeholder"}, decoy)
        res = (doc or {}).get("results") or []
        by = {r.get("branch"): r for r in res}
        ck("★S5 non-fast-forward 거부 → 그 브랜치만 실패(HINT_PUSH_FAILED) · 다음 브랜치는 시도해 성공 · rc=5",
           rc == EXIT_PUSH_FAILED and by.get("single-node", {}).get("returncode") not in (0, None)
           and by.get("single-node", {}).get("code") == "HINT_PUSH_FAILED"
           and by.get("multi-node", {}).get("returncode") == 0 and by.get("multi-node", {}).get("code") is None,
           (rc, res))
        ck("★강제 push 없음: 원격 single-node 는 그대로",
           g("rev-parse", "refs/heads/single-node", cwd=remote) == remote_single_before)
        ck("★대상 저장소 기준 적재: CWD 가 다른 저장소여도 --repo 의 hintlib 으로 --repo 의 커밋을 민다",
           g("rev-parse", "refs/heads/multi-node", cwd=remote) == local_multi
           and all((r.get("path") or "").startswith("hintlib.tag.git_push_authenticated") for r in res), res)
        ck("★followTags=true 에서도 브랜치 push 가 태그를 딸려 보내지 않는다(last-good 유출 ✗)",
           refs(remote, "refs/tags") == [], refs(remote, "refs/tags"))

        # C: 스킴 인식 -- 로컬 경로 원격은 토큰 없이 민다(옛 경로는 SSH·로컬이어도 토큰 부재로 죽었다 · H7)
        local_remote = bare("local.git")
        rc, doc, _ = run(here, str(local_remote), {}, tdp)
        res = (doc or {}).get("results") or []
        ck("★H7 로컬(비-https) 원격 · 토큰 없음 → 두 브랜치 모두 push 성공 · rc=0",
           rc == EXIT_OK and len(res) == 2 and all(r.get("returncode") == 0 and r.get("code") is None
                                                  and (r.get("path") or "").startswith("hintlib.tag.") for r in res),
           (rc, res))
        ck("C: 태그 딸림 없음", refs(local_remote, "refs/tags") == [], refs(local_remote, "refs/tags"))

        # 변이 음성대조 ①: HintError 처리 절을 걷어낸 사본은 사유코드 분류를 잃는다(범용 절이 JSON 은 지킨다)
        src = here.read_text(encoding="utf-8")
        needle = "        except hint_error as exc:\n"
        if src.count(needle) != 1:
            ck("변이 앵커(except hint_error) 1건", False, src.count(needle))
        else:
            mutated = tdp / "push_branches_mutated.py"
            mutated.write_text(src.replace(needle, "        except KeyboardInterrupt as exc:\n"), encoding="utf-8")
            rc_m, doc_m, _ = run(mutated, "fx", {}, tdp)
            res_m = (doc_m or {}).get("results") or []
            ck("★변이 음성대조: HintError 를 따로 잡지 않으면 사유코드(code)·경로 분류가 사라진다(결함 재현)",
               rc_m == EXIT_PUSH_FAILED and len(res_m) == 2
               and all(r.get("code") is None and "HintError(" not in (r.get("path") or "") for r in res_m),
               (rc_m, res_m))

        # D: 폴백 -- 대상 저장소에 hintlib 이 없으면 표시된 폴백으로 민다(경고 · 범위 옵션 유지)
        parked = tdp / "hintlib.parked"
        shutil.move(str(fx_hintlib), str(parked))
        try:
            fb_remote = bare("fallback.git")
            rc, doc, err_txt = run(here, str(fb_remote), {}, tdp)
            res = (doc or {}).get("results") or []
            ck("D 폴백: 경로에 폴백·사유 표시 · stderr 경고 · rc=0",
               rc == EXIT_OK and len(res) == 2
               and all("fallback" in (r.get("path") or "") and r.get("returncode") == 0 for r in res)
               and "hintlib 미적재" in err_txt, (rc, res, err_txt[-300:]))
            ck("★D 폴백도 followTags 로 태그를 딸려 보내지 않는다(--no-follow-tags)",
               refs(fb_remote, "refs/tags") == [], refs(fb_remote, "refs/tags"))
            # 변이 음성대조 ②: 폴백의 범위 옵션을 비우면 태그가 새어 나간다(위 ★가 공허하지 않다)
            scope_line = 'FALLBACK_PUSH_SCOPE_ARGS = ("--no-follow-tags", "--no-recurse-submodules")\n'
            if src.count(scope_line) != 1:
                ck("변이 앵커(FALLBACK_PUSH_SCOPE_ARGS) 1건", False, src.count(scope_line))
            else:
                mutated2 = tdp / "push_branches_mutated_scope.py"
                mutated2.write_text(src.replace(scope_line, "FALLBACK_PUSH_SCOPE_ARGS = ()\n"), encoding="utf-8")
                leak_remote = bare("fallback-leak.git")
                run(mutated2, str(leak_remote), {}, tdp)
                ck("★변이 음성대조: 폴백 범위 옵션을 비우면 last-good 태그가 원격에 올라간다(결함 재현)",
                   "refs/tags/last-good-fixture" in refs(leak_remote, "refs/tags"), refs(leak_remote, "refs/tags"))
        finally:
            shutil.move(str(parked), str(fx_hintlib))

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
    if args.remote.startswith("-"):
        # 폴백 경로의 맨 `git push` 에서 옵션으로 읽힌다(옵션 주입 ✗ · hintlib 경로는 자체적으로 거부한다).
        print(f"[push_branches] FAIL: 원격 인자가 옵션처럼 보인다: {args.remote!r}", file=sys.stderr)
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
