#!/usr/bin/env python3
"""check_sub_hint_equip.py — single 서브 hint 발행 장착의 **그 노드 쪽** 결정론 판정·장착 도구.

2026-10-11 · plan_26101104 Step 3 · 사용자 결정(single 서브 한정 자체 발행).

왜 있는가
  single 서브(sub_mode a2a-agent)는 자기가 잰 셀을 스스로 hint 태그로 발행한다. 장착은 세 가지다:
    ① hint 클론 — 워크스페이스 git **밖**의 별도 저장소(origin = 프로젝트 원격 · 브랜치 hint).
       워크스페이스 git 은 local-only 그대로다(policy SUB_GIT_LOCAL_ONLY — 그 정책은 워크스페이스 git 을 다스린다).
    ② push 역할 선언 `hints/.push_authority` — hint.py 가 도는 체크아웃(= 서브 워크스페이스)에 둔다. 비추적이며
       배달되지 않는다(각 체크아웃이 스스로 하는 선언 · 카탈로그 권위 `hints/.central_authority` 는 메인 단독).
    ③ push 토큰 `GITHUB_TOKEN` — **사람이** 그 노드의 `envs/.env` 에 둔다(비밀은 배달 평면 ✗). 이 도구는 **존재만**
       본다 — 값은 읽어서 비교하지도, 출력하지도 않는다.
  메인은 서브를 스캔하지 않는다(헌법 노드 제어 ①). 그래서 이 판정은 서브가 돌리고 JSON 을 testlog 로 보고한다.

선언 자리(단일)
  · hint 클론 경로 = 서브 워크스페이스 `envs/.env` 의 `HINT_CLONE_DIR=<절대경로>` 한 줄. 워크스페이스 밖이어야 한다.
    (`envs/.env` 는 서브가 저작하는 비추적 평면이고 배달되지 않는다 — 토큰과 같은 파일이라 "그 노드의 hint 설정" 이
    한 자리에 모인다. 메인에는 이 값이 없다 — 그 노드의 경로이므로.)
  · 토큰 = 같은 파일의 `GITHUB_TOKEN`(또는 환경변수 · hint.py `tag.read_push_token` 과 같은 규칙).

동사
  (기본)                 판정만 — JSON 1개를 stdout 에(비밀 미출력) · exit 0=ready · 3=미장착 항목 있음
  --clone --remote-url U  ① 장착: HINT_CLONE_DIR 에 `git clone --single-branch --branch hint --depth 1 --no-tags U`
  --write-push-authority  ② 장착: `hints/.push_authority` 작성(그 경로가 git-ignore 되지 않으면 거부 — 추적되면 배포된다)
  --self-test             격리 임시 저장소로 양·음성 사례

종료코드: 0=ready(또는 장착 동사 성공) · 2=사용 오류/선언 결함 · 3=미장착(판정 정상) · 4=장착 실패
stdlib 만. 출처 표시: 출력의 `provenance` = "measured"(이 노드에서 실제로 본 값) — 추정·폴백 값은 싣지 않는다.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
# 배달 위치 = <ws>/.claude/skills/terraforming_node/scripts/ → 워크스페이스 루트는 4단 위(메인 정본 위치에서도 같다).
DEFAULT_WORKSPACE = HERE.parents[3]

ENV_FILE_REL = "envs/.env"
CLONE_DIR_KEY = "HINT_CLONE_DIR"
TOKEN_KEY = "GITHUB_TOKEN"            # hintlib.tag.TOKEN_KEY 와 같은 이름(자체검사가 교차검증)
PUSH_FLAG_REL = "hints/.push_authority"
CENTRAL_FLAG_REL = "hints/.central_authority"
PII_TERMS_REL = ".claude/pii_terms.txt"
HINT_BRANCH = "hint"
# 사람 수행지시서 템플릿(메인이 docs/request/ 로 발행 — 서브에는 docs/request/ 가 없다 · docs.md §서브 docs 계약).
REQUEST_TEMPLATE_REL = ".claude/skills/terraforming_node/templates/requests/sub_hint_push_equip.md"

EXIT_READY, EXIT_USAGE, EXIT_NOT_READY, EXIT_EQUIP_FAILED = 0, 2, 3, 4
_CRED_IN_URL = re.compile(r"^[a-z][a-z0-9+.-]*://[^/@]+@", re.I)


class UsageError(Exception):
    pass


# ── 판독 ─────────────────────────────────────────────────────────────────────────────────────
def read_env_key(ws: Path, key: str) -> str | None:
    """`envs/.env` 의 KEY 값(`#` 주석 · 빈 값 = 없음 · 양끝 따옴표 제거). hint.py `tag.read_push_token` 과 같은 규칙.
    파일이 있는데 읽을 수 없으면 부재가 아니라 결함이다(UsageError)."""
    f = ws / ENV_FILE_REL
    if not f.is_file():
        return None
    try:
        text = f.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as e:
        raise UsageError(f"{ENV_FILE_REL} 를 읽을 수 없다({type(e).__name__}) — 값은 출력하지 않는다")
    for line in text.splitlines():
        st = line.strip()
        if st.startswith("#") or not st.startswith(key + "="):
            continue
        val = st.split("=", 1)[1].strip().strip('"').strip("'")
        if val:
            return val
    return None


def _git(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    # cwd and its on-disk Git metadata are the observation authority. Inherited
    # repository/index/config injection must not redirect a local readiness read.
    # Keep transport/authentication settings such as GIT_SSH_COMMAND unchanged.
    redirected = {"GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR", "GIT_INDEX_FILE",
                  "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES"}
    env = {k: v for k, v in os.environ.items()
           if k not in redirected and not k.startswith("GIT_CONFIG")}
    env.update(GIT_TERMINAL_PROMPT="0", GIT_OPTIONAL_LOCKS="0")
    return subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True, env=env)


def _scheme(url: str) -> str:
    if url.startswith(("https://",)):
        return "https"
    if url.startswith("http://"):
        return "http"
    if url.startswith("ssh://") or re.match(r"^[^/:@]+@[^/:]+:", url):
        return "ssh"
    return "local"


def _inside(child: Path, parent: Path) -> bool:
    try:
        child.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def check_clone(ws: Path) -> dict:
    raw = read_env_key(ws, CLONE_DIR_KEY)
    out = {"path_source": f"{ENV_FILE_REL}:{CLONE_DIR_KEY}", "declared": raw is not None}
    if raw is None:
        return dict(out, status="missing", reason=f"{CLONE_DIR_KEY} 미선언 — {ENV_FILE_REL} 에 절대경로 한 줄을 둔다")
    p = Path(raw)
    if not p.is_absolute():
        return dict(out, status="invalid", reason=f"{CLONE_DIR_KEY} 는 절대경로여야 한다")
    if _inside(p, ws):
        return dict(out, status="invalid",
                    reason="hint 클론이 워크스페이스 안이다 — 워크스페이스 git 밖이어야 한다(SUB_GIT_LOCAL_ONLY)")
    if not p.is_dir():
        return dict(out, status="missing", reason="선언된 경로에 클론이 없다 — --clone 으로 장착한다")
    top = _git(p, "rev-parse", "--show-toplevel")
    if top.returncode != 0 or Path(top.stdout.strip()).resolve() != p.resolve():
        return dict(out, status="invalid", reason="선언된 경로가 git 저장소 루트가 아니다")
    url = _git(p, "remote", "get-url", "origin")
    if url.returncode != 0 or not url.stdout.strip():
        return dict(out, status="invalid", reason="hint 클론에 origin 이 없다", origin_scheme="absent")
    u = url.stdout.strip()
    if _CRED_IN_URL.match(u):
        # URL 자체는 출력하지 않는다(자격증명이 들어 있다).
        return dict(out, status="invalid", reason="origin URL 에 자격증명이 박혀 있다 — 토큰은 envs/.env 로만",
                    origin_scheme=_scheme(u))
    ref = _git(p, "rev-parse", "--verify", "--quiet", f"refs/remotes/origin/{HINT_BRANCH}")
    if ref.returncode != 0:
        ref = _git(p, "rev-parse", "--verify", "--quiet", f"refs/heads/{HINT_BRANCH}")
    hint_ref = "present" if ref.returncode == 0 else "absent"
    st = "ok" if hint_ref == "present" and _scheme(u) != "http" else "invalid"
    reason = None if st == "ok" else ("hint 브랜치 ref 가 없다" if hint_ref == "absent" else "평문 http 원격은 거부한다")
    # Local observation only. This does not approve paths or prove native sandbox
    # write access. Linked worktrees can keep both metadata directories elsewhere.
    metadata = {}
    for key, args in (("git_dir", ("rev-parse", "--absolute-git-dir")),
                      ("git_common_dir", ("rev-parse", "--path-format=absolute", "--git-common-dir"))):
        observed = _git(p, *args)
        value = observed.stdout.strip()
        metadata[key] = str(Path(value).resolve()) if observed.returncode == 0 and Path(value).is_absolute() else None
    return dict(out, status=st, reason=reason, origin_scheme=_scheme(u), hint_ref=hint_ref,
                hint_clone_dir=str(p.resolve()), observed_home_dir=str(Path.home().resolve()),
                **metadata)


def _ignored(ws: Path, rel: str) -> bool | None:
    r = _git(ws, "check-ignore", "-q", "--", rel)
    if r.returncode == 0:
        return True
    if r.returncode == 1:
        return False
    return None                              # 판독 불가(워크스페이스가 git 이 아님 등) — 호출부가 결함으로 다룬다


def check_push_authority(ws: Path) -> dict:
    ign = _ignored(ws, PUSH_FLAG_REL)
    tracked = _git(ws, "ls-files", "--error-unmatch", "--", PUSH_FLAG_REL).returncode == 0
    if tracked or ign is False:
        return {"status": "tracked_risk", "ignored": ign, "tracked": tracked,
                "reason": "역할 선언이 추적 대상이다 — 커밋되면 배포·복사된다(.gitignore 에 hints/.push_authority)"}
    if ign is None:
        return {"status": "invalid", "ignored": None, "reason": "git check-ignore 판독 불가 — 워크스페이스 git 을 확인한다"}
    if not (ws / PUSH_FLAG_REL).is_file():
        return {"status": "missing", "ignored": True, "reason": "--write-push-authority 로 장착한다"}
    return {"status": "ok", "ignored": True}


def check_token(ws: Path, origin_scheme: str | None, environ=None) -> dict:
    environ = os.environ if environ is None else environ
    if str(environ.get(TOKEN_KEY) or "").strip():
        src = "env"
    elif read_env_key(ws, TOKEN_KEY) is not None:
        src = ENV_FILE_REL
    else:
        src = None
    if src is not None:
        return {"status": "present", "source": src}
    if origin_scheme in ("ssh", "local"):
        return {"status": "not_required", "source": None,
                "reason": f"origin 이 {origin_scheme} 이다 — https 원격만 토큰이 필요하다"}
    return {"status": "absent", "source": None,
            "reason": f"{TOKEN_KEY} 가 환경변수에도 {ENV_FILE_REL} 에도 없다 — 사람이 그 노드에 둔다(배달 ✗)"}


def evaluate(ws: Path, environ=None) -> dict:
    ws = ws.resolve()
    clone = check_clone(ws)
    remotes = _git(ws, "remote")
    if remotes.returncode != 0:
        local_only = {"status": "invalid", "reason": "워크스페이스 git 판독 불가"}
    else:
        n = len([x for x in remotes.stdout.split() if x.strip()])
        local_only = {"status": "ok" if n == 0 else "violation", "remotes": n}
    checks = {
        "hint_clone": clone,
        "push_authority": check_push_authority(ws),
        "central_authority": ({"status": "present_violation",
                               "reason": "카탈로그 권위는 메인 단독이다 — 서브에 이 선언이 있으면 안 된다"}
                              if (ws / CENTRAL_FLAG_REL).exists() else {"status": "absent"}),
        "token": check_token(ws, clone.get("origin_scheme"), environ),
        "pii_terms": {"status": "present" if (ws / PII_TERMS_REL).is_file() else "absent"},
        "workspace_git_local_only": local_only,
    }
    good = {"hint_clone": ("ok",), "push_authority": ("ok",), "central_authority": ("absent",),
            "token": ("present", "not_required"), "pii_terms": ("present",),
            "workspace_git_local_only": ("ok",)}
    ready = all(checks[k]["status"] in v for k, v in good.items())
    actions = []
    if checks["hint_clone"]["status"] != "ok":
        actions.append({"actor": "sub-agent", "step": "clone",
                        "how": f"{ENV_FILE_REL} 에 {CLONE_DIR_KEY}=<워크스페이스 밖 절대경로> 를 두고 "
                               "`--clone --remote-url <메인이 Task Message 로 준 프로젝트 원격>`"})
    if checks["push_authority"]["status"] != "ok":
        actions.append({"actor": "sub-agent", "step": "push_authority", "how": "--write-push-authority"})
    if checks["token"]["status"] == "absent" or checks["pii_terms"]["status"] == "absent":
        actions.append({"actor": "human", "step": "place_secrets",
                        "what": [x for x, c in ((TOKEN_KEY + "@" + ENV_FILE_REL, checks["token"]["status"] == "absent"),
                                                (PII_TERMS_REL, checks["pii_terms"]["status"] == "absent")) if c],
                        "request_template": REQUEST_TEMPLATE_REL,
                        "how": "testlog 로 보고만 한다 — 메인이 docs/request/ 수행지시서를 발행하고 사람이 둔다"})
    if checks["central_authority"]["status"] != "absent" or checks["workspace_git_local_only"]["status"] != "ok":
        actions.append({"actor": "main", "step": "escalate",
                        "how": "역할·원격 위반은 서브가 스스로 고치지 않는다 — 보고하고 메인 HITL"})
    return {"schema_version": 1, "tool": "check_sub_hint_equip", "provenance": "measured",
            "checks": checks, "ready": ready, "next_actions": actions}


# ── 장착 ─────────────────────────────────────────────────────────────────────────────────────
def equip_clone(ws: Path, remote_url: str) -> str:
    if not remote_url or _CRED_IN_URL.match(remote_url):
        raise UsageError("--remote-url 이 비었거나 자격증명이 박혀 있다 — 토큰은 envs/.env 로만")
    if _scheme(remote_url) == "http":
        raise UsageError("평문 http 원격은 거부한다")
    raw = read_env_key(ws, CLONE_DIR_KEY)
    if raw is None:
        raise UsageError(f"{CLONE_DIR_KEY} 미선언 — {ENV_FILE_REL} 에 워크스페이스 밖 절대경로 한 줄을 먼저 둔다(선언 자리는 하나)")
    p = Path(raw)
    if not p.is_absolute() or _inside(p, ws):
        raise UsageError(f"{CLONE_DIR_KEY} 는 워크스페이스 밖 절대경로여야 한다(SUB_GIT_LOCAL_ONLY — 워크스페이스 git 에 원격 ✗)")
    if p.exists() and any(p.iterdir()):
        st = check_clone(ws)
        if st["status"] == "ok":
            return "already"                 # 멱등: 이미 장착됐다
        raise UsageError(f"{CLONE_DIR_KEY} 경로가 비어 있지 않고 유효한 hint 클론도 아니다 — 사람이 정리한다(자동 삭제 ✗)")
    p.parent.mkdir(parents=True, exist_ok=True)
    r = subprocess.run(["git", "clone", "--single-branch", "--branch", HINT_BRANCH, "--depth", "1", "--no-tags",
                        "--quiet", remote_url, str(p)], capture_output=True, text=True,
                       env=dict(os.environ, GIT_TERMINAL_PROMPT="0"))
    if r.returncode != 0:
        raise RuntimeError(f"git clone rc={r.returncode}: {r.stderr.strip()[:300]}")
    return "cloned"


def equip_push_authority(ws: Path, *, utc: str | None) -> str:
    st = check_push_authority(ws)
    if st["status"] == "ok":
        return "already"
    if st["status"] != "missing":
        raise UsageError(f"push 역할 선언을 쓰지 않는다 — {st.get('reason')}")
    f = ws / PUSH_FLAG_REL
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text("# hint 태그 push 역할 선언(비추적 · 배달 ✗ · 카탈로그 권위 아님) — check_sub_hint_equip.py 가 썼다"
                 + (f" · {utc}" if utc else "") + "\n", encoding="utf-8")
    return "written"


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def _self_test() -> int:
    fails: list[str] = []
    ran = [0]

    def ck(name, cond, detail=""):
        ran[0] += 1
        print(f"  [{'PASS' if cond else 'FAIL'}] {name}" + ("" if cond else f" — {detail}"))
        if not cond:
            fails.append(name)

    env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t.invalid", GIT_COMMITTER_NAME="t",
               GIT_COMMITTER_EMAIL="t@t.invalid", GIT_TERMINAL_PROMPT="0")

    def g(cwd, *a):
        subprocess.run(["git", *a], cwd=str(cwd), check=True, capture_output=True, text=True, env=env)

    secret = "ghp_SELFTEST_SECRET_VALUE_9f2c"
    with tempfile.TemporaryDirectory(prefix="hint-equip-") as td:
        td = Path(td)
        # 원격(프로젝트 원격 흉내 · 로컬 경로) — hint 브랜치에 안내 커밋 1개.
        origin = td / "origin.git"
        seed = td / "seed"
        seed.mkdir()
        g(seed, "init", "-q", "-b", HINT_BRANCH)
        (seed / "README.md").write_text("<!-- hint-branch-format: v7 -->\n", encoding="utf-8")
        g(seed, "add", "README.md")
        g(seed, "commit", "-q", "-m", "guide")
        g(td, "clone", "-q", "--bare", str(seed), str(origin))
        # 서브 워크스페이스 흉내 — local-only git · .gitignore 가 마커를 덮는다.
        ws = td / "ws"
        (ws / "envs").mkdir(parents=True)
        g(ws, "init", "-q")
        (ws / ".gitignore").write_text("envs/.env\nhints/.push_authority\nhints/.drafts/\n.claude/pii_terms.txt\n",
                                       encoding="utf-8")
        clone_dir = td / "hint-clone"
        r0 = evaluate(ws, environ={})
        ck("미장착 → ready=False · 클론 missing(미선언) · 토큰 absent · 마커 missing",
           not r0["ready"] and r0["checks"]["hint_clone"]["status"] == "missing"
           and r0["checks"]["hint_clone"]["declared"] is False
           and r0["checks"]["token"]["status"] == "absent"
           and r0["checks"]["push_authority"]["status"] == "missing", r0)
        ck("미장착 → 다음 행동에 사람 요청(템플릿 경로 포함)",
           any(a["actor"] == "human" and a["request_template"] == REQUEST_TEMPLATE_REL for a in r0["next_actions"]))
        # 선언 없이 --clone → 거부(선언 자리는 하나)
        try:
            equip_clone(ws, str(origin))
            ck("미선언 --clone 거부", False, "통과했다")
        except UsageError:
            ck("미선언 --clone 거부", True)
        # 워크스페이스 안 경로 선언 → 거부
        (ws / "envs" / ".env").write_text(f"{CLONE_DIR_KEY}={ws / 'inner'}\n", encoding="utf-8")
        try:
            equip_clone(ws, str(origin))
            ck("워크스페이스 안 클론 경로 거부", False, "통과했다")
        except UsageError:
            ck("워크스페이스 안 클론 경로 거부", True)
        ck("워크스페이스 안 선언 → 판정 invalid", check_clone(ws)["status"] == "invalid")
        # 자격증명 박힌 URL 거부
        (ws / "envs" / ".env").write_text(f"{CLONE_DIR_KEY}={clone_dir}\n", encoding="utf-8")
        try:
            equip_clone(ws, "https://user:tok@example.invalid/x.git")
            ck("자격증명 박힌 URL 거부", False, "통과했다")
        except UsageError:
            ck("자격증명 박힌 URL 거부", True)
        # 정상 장착
        ck("--clone 정상(로컬 원격)", equip_clone(ws, str(origin)) == "cloned" and clone_dir.is_dir())
        ck("--clone 멱등(already)", equip_clone(ws, str(origin)) == "already")
        ck("--write-push-authority 정상", equip_push_authority(ws, utc="2026-10-11T00:00:00Z") == "written")
        ck("--write-push-authority 멱등", equip_push_authority(ws, utc=None) == "already")
        (ws / ".claude").mkdir(exist_ok=True)
        (ws / PII_TERMS_REL).write_text("", encoding="utf-8")   # 빈 파일 = "리터럴 없음" 선언(존재)
        r1 = evaluate(ws, environ={})
        ck("로컬 원격 · 마커 · pii 있음 → ready(토큰 not_required)",
           r1["ready"] and r1["checks"]["token"]["status"] == "not_required"
           and r1["checks"]["hint_clone"]["hint_ref"] == "present", r1)
        # https 원격으로 바꾸면 토큰이 필요하다
        g(clone_dir, "remote", "set-url", "origin", "https://example.invalid/proj.git")
        r2 = evaluate(ws, environ={})
        ck("https 원격 · 토큰 없음 → absent · ready=False", not r2["ready"]
           and r2["checks"]["token"]["status"] == "absent", r2["checks"]["token"])
        with open(ws / "envs" / ".env", "a", encoding="utf-8") as fh:
            fh.write(f"# 주석\n{TOKEN_KEY}='{secret}'\n")
        r3 = evaluate(ws, environ={})
        dump = json.dumps(r3, ensure_ascii=False)
        ck("토큰 present(envs/.env) · ready", r3["ready"] and r3["checks"]["token"] == {"status": "present",
                                                                                       "source": ENV_FILE_REL}, r3)
        ck("★토큰 값은 출력 어디에도 없다", secret not in dump)
        ck("환경변수 토큰 → source=env", evaluate(ws, environ={TOKEN_KEY: "x"})["checks"]["token"]["source"] == "env")
        # hintlib.tag 와 같은 판독 규칙인가(교차검증 — 단일 소유 불가 시 차선 · workflow.md §결정론 규율)
        tag_mod = None
        hl = DEFAULT_WORKSPACE / ".claude/skills/hint-publisher/scripts"
        if (hl / "hintlib" / "tag.py").is_file():
            sys.path.insert(0, str(hl))
            try:
                from hintlib import tag as tag_mod  # type: ignore
            except Exception:                    # 적재 실패는 아래에서 FAIL 로 드러낸다
                tag_mod = None
            finally:
                sys.path.pop(0)
        ck("hintlib.tag 와 토큰 키·파일·판독 규칙 일치(교차검증)",
           tag_mod is not None and tag_mod.TOKEN_KEY == TOKEN_KEY and tag_mod.TOKEN_ENV_FILE_REL == ENV_FILE_REL
           and tag_mod.read_push_token(ws, environ={}) == secret, "hintlib 적재 실패 또는 규칙 불일치")
        # 자격증명 박힌 origin → invalid · URL 은 출력되지 않는다
        g(clone_dir, "remote", "set-url", "origin", f"https://x:{secret}@example.invalid/p.git")
        r4 = evaluate(ws, environ={})
        ck("origin URL 자격증명 → invalid · 출력 무비밀", r4["checks"]["hint_clone"]["status"] == "invalid"
           and secret not in json.dumps(r4))
        g(clone_dir, "remote", "set-url", "origin", "https://example.invalid/proj.git")
        # 역할 위반: 카탈로그 권위 선언이 서브에 있으면 위반
        (ws / CENTRAL_FLAG_REL).write_text("x\n", encoding="utf-8")
        r5 = evaluate(ws, environ={})
        ck("서브의 .central_authority → present_violation · ready=False · 메인 에스컬레이션",
           not r5["ready"] and r5["checks"]["central_authority"]["status"] == "present_violation"
           and any(a["actor"] == "main" for a in r5["next_actions"]))
        (ws / CENTRAL_FLAG_REL).unlink()
        # 워크스페이스에 원격이 붙으면 위반(SUB_GIT_LOCAL_ONLY)
        g(ws, "remote", "add", "origin", str(origin))
        r6 = evaluate(ws, environ={})
        ck("워크스페이스 원격 → violation(SUB_GIT_LOCAL_ONLY)",
           not r6["ready"] and r6["checks"]["workspace_git_local_only"] == {"status": "violation", "remotes": 1})
        g(ws, "remote", "remove", "origin")
        # 마커가 ignore 되지 않는 워크스페이스 → 쓰기 거부(추적되면 배포된다)
        ws2 = td / "ws2"
        ws2.mkdir()
        g(ws2, "init", "-q")
        try:
            equip_push_authority(ws2, utc=None)
            ck("비-ignore 워크스페이스 마커 쓰기 거부", False, "통과했다")
        except UsageError:
            ck("비-ignore 워크스페이스 마커 쓰기 거부", (not (ws2 / PUSH_FLAG_REL).exists()))
        ck("판정 결정론(같은 입력 → 같은 JSON)",
           json.dumps(evaluate(ws, environ={}), sort_keys=True) == json.dumps(evaluate(ws, environ={}), sort_keys=True))
    # 요청 템플릿은 **메인 전용**이다(수행지시서 발행 = 메인 몫 · docs.md 서브 docs 계약). 서브 렌더 트리에는 terraforming_node 의
    #   scripts/ 만 가므로 거기서 부재는 정상이다(2026-10-11 실서브 적용에서 자체검사 21/22 의 원인). 메인 판별 = 스킬 문서 실재.
    _main_tree = (DEFAULT_WORKSPACE / ".claude/skills/terraforming_node/SKILL.md").is_file()
    ck("사람 요청 템플릿이 실재한다(메인 트리 · 서브 렌더 트리는 메인 전용이라 부재 정상)",
       (DEFAULT_WORKSPACE / REQUEST_TEMPLATE_REL).is_file() or not _main_tree, REQUEST_TEMPLATE_REL)
    print(f"--- {ran[0] - len(fails)}/{ran[0]} PASS")
    return 0 if not fails else 1


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="single 서브 hint 발행 장착 판정·장착(비밀 미출력)")
    ap.add_argument("--workspace", default=str(DEFAULT_WORKSPACE), help="서브 워크스페이스 루트(기본: 이 스크립트 위치에서 파생)")
    ap.add_argument("--clone", action="store_true", help="① hint 클론 장착(HINT_CLONE_DIR 선언 필수)")
    ap.add_argument("--remote-url", help="--clone 의 프로젝트 원격(메인이 Task Message 로 준다 · 자격증명 박힌 URL 거부)")
    ap.add_argument("--write-push-authority", action="store_true", help="② hints/.push_authority 작성")
    ap.add_argument("--utc", help="--write-push-authority 의 선언 시각(주입 · 생략 가능)")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args(argv)
    if a.self_test:
        return _self_test()
    ws = Path(a.workspace).resolve()
    try:
        if a.clone:
            print(json.dumps({"equip": "clone", "result": equip_clone(ws, a.remote_url or "")}, ensure_ascii=False))
        if a.write_push_authority:
            print(json.dumps({"equip": "push_authority", "result": equip_push_authority(ws, utc=a.utc)},
                             ensure_ascii=False))
        if a.clone or a.write_push_authority:
            return EXIT_READY
        res = evaluate(ws)
    except UsageError as e:
        print(f"[check_sub_hint_equip] FAIL: {e}", file=sys.stderr)
        return EXIT_USAGE
    except RuntimeError as e:
        print(f"[check_sub_hint_equip] FAIL(장착): {e}", file=sys.stderr)
        return EXIT_EQUIP_FAILED
    print(json.dumps(res, ensure_ascii=False, indent=2))
    return EXIT_READY if res["ready"] else EXIT_NOT_READY


if __name__ == "__main__":
    sys.exit(main())
