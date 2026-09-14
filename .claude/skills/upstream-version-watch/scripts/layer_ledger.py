#!/usr/bin/env python3
"""layer_ledger.py -- 분류표 원장과 공통층 동기 지점 (policy BRANCH_CONSTITUTION_LAYERING)

사용법:
  layer_ledger.py common-layer  --repo .                       공통층 pathspec 전수(동기화 대상)
  layer_ledger.py sync-point    --repo . --branch <Z> [--counterpart <W>] [--ledger-ref <ref>]...
                                                               Z 의 마지막 공통층 동기 지점 커밋
  layer_ledger.py divergence    --repo . --branch <Z> [--counterpart <W>] [--ledger-ref <ref>]... [--since <c>]
                                                               동기 지점 이후 Z 의 공통층 갈라짐
  layer_ledger.py validate      --entry-file <json>            분류표 항목 계약 검사(쓰지 않는다)
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

원장을 어디서 읽나 -- 복제하지 않고 ref 에서 읽는다 (2026-09-14 · ⑧-pre D2 S1)
-----------------------------------------------------------------------------
원장은 동기화 제외라 **대상 브랜치 워크트리에는 없을 수 있다**(2026-09-14 실측: single-node 에 원장 0 ·
그래서 multi→single 방향의 갈라짐 검사는 늘 `empty-ledger · 판정 불가` 였고 구조적으로 울릴 수 없었다).
처방은 원장을 대상에 복사하는 것이 아니라(두 번째 자리 = policy:GIT_SINGLE_AUTHORITY 위반) **판정 시점에
출발 브랜치 ref 에서 git 이 든 바이트를 읽는 것**이다. `--ledger-ref <ref>` 를 주면 워크트리 원장과 그
ref 들의 원장을 entry_id 합집합으로 **읽기만** 한다(쓰지 않는다). 같은 id 가 다른 내용이면 append-only 가
어딘가에서 깨진 것이므로 합치지 않고 `ledger-conflict` 로 드러낸다. 읽을 수 없는 원장·풀리지 않는 ref 는
빈 원장으로 접지 않고 `ledger-unreadable` 로 드러낸다(부재 ≠ 결측).

동기 지점을 해시로 적지 않는 이유
---------------------------------
한 항목은 *출발* 브랜치와 그 커밋만 적는다. 대상 브랜치에서의 대응 커밋은 **탐색으로 찾는다**. 두 자리에
해시를 적어 두면 한쪽이 rebase·amend 될 때 조용히 거짓이 되고, 커밋 메시지로 찾으면 이름을 바꾸는 순간
술어가 눈이 먼다. 트리 동일성은 이름과 무관하다.

동기 지점 = 실제로 내려앉은 공통층 트리 (2026-09-14 · ⑧-pre D2 S2)
------------------------------------------------------------------
종전 탐색은 "대상 커밋 중 **원장에 적힌 출발 커밋**과 공통층 트리가 같은 것" 이었다. 그런데 원장 append 는
멱등이라 같은 분류표로 **재실행한 회차**(④ 성공 → ⑤ RED → 교정 커밋 → 재실행)는 새 항목을 만들지 않고,
실제로 내려앉는 트리는 교정 뒤의 출발 HEAD 다. 2026-09-12 가 그랬다 -- 원장은 7e0e3a1 을 적었고
single-node 에 내려앉은 것은 15129f0 의 트리(3766721)였다. 그래서 `divergence --branch single-node` 가
영원히 no-sync-point 였다. 게다가 종료 시퀀스는 ④ 에서 **이번 항목을 먼저 적은 뒤** ⑤ 갈라짐 검사를 돌므로,
"마지막 항목만 본다" 는 규칙은 아직 착지하지 않은 항목을 보고 늘 판정 불가였다.

그래서 이제는 **원장 항목을 새것부터 거꾸로** 훑고, 항목마다 "출발 커밋 이후 출발 브랜치 계보의 커밋 중
하나와 공통층 트리가 같은 대상 커밋" 을 찾는다(착지 = 트리 동치). 처음 찾은 착지가 동기 지점이다.
과거 항목은 고치지 않는다(append-only) -- 기록과 착지의 차이는 `departure_drift` 로 **표시**한다.
대상 커밋 여럿이 계보와 맞으면 **계보상 가장 새 출발 커밋**에 맞는 것을 착지로 삼는다 -- 동기화는 대상을
출발 계보의 앞쪽으로만 옮기므로, 옛 출발 트리와 같아진 대상 커밋은 착지가 아니라 **대상 쪽 되돌림**이다
(그것을 착지로 오인하면 되돌림이 기준선에 묻혀 덮어쓰기가 통과한다 · 2026-09-14 ⑧-pre D2 리뷰 교정).

갈라짐 = "대상이 바꿨고" ∧ "덮으면 사라지고" ∧ "상대가 아직 받지 않았다" (2026-09-14 · ⑧-pre D2 리뷰 교정)
------------------------------------------------------------------------------------------------
동기 지점 이후 대상이 바꾼 파일을 **전부** 갈라짐으로 세면, 처방("출발 브랜치로 옮긴 뒤 다시")을 따라도
절대 풀리지 않는다 -- 옮긴 뒤에도 대상 쪽 diff 는 그대로이기 때문이다(안내대로 했는데 막히는 것은 교착이다).
그래서 `--counterpart` 가 있으면 동기 지점 이후 바뀐 파일 중 아래 둘을 **흡수**로 뺀다(`absorbed` 로 표시):
  - 상대와 바이트가 같다(동기화가 바꿀 것이 없다)
  - 대상의 바이트(삭제 포함)가 **동기 지점 이후 상대 계보**에 들어왔다(cherry-pick 으로 옮겨졌고 상대가 그 뒤 더 고쳤다)
동기 지점 **이전** 바이트로의 되돌림은 흡수가 아니다 -- 상대 계보의 옛 이력은 옮겨진 증거가 아니다.

원장 무결성이 깨졌으면 판정하지 않는다
--------------------------------------
`ledger-conflict`(같은 id 다른 내용) · `ledger-unreadable`(못 읽는 원장 · 풀리지 않는 ref) 은 "동기 지점 없음" 이
아니다. 그것을 blob 조상(더 약한 판정)으로 접으면 원장이 말하는 갈라짐을 약한 판정이 clean 으로 덮는다. 그래서
갈라짐 판정은 `unjudged`(basis=`ledger-integrity`) 로 멈춘다 -- 동기화는 COMMON_LAYER_UNJUDGED 로 선다.
====

동기 지점이 없을 때 -- 판정 불가를 통과로 두지 않는다 (2026-09-14 · ⑧-pre D2 S1)
------------------------------------------------------------------------------
원장이 비었거나 어느 항목도 착지하지 않았으면 기준 커밋이 없다. 그때 `--counterpart` 가 있으면 **blob 조상
판정**으로 대신한다. 대상의 공통층 파일 중 상대와 다른 것마다, 대상이 그 경로를 가진 적이 있고 아래 중 하나면
덮으면 사라지는 대상 전용 변경이다(= diverged):
  ⓐ 대상의 바이트가 상대 이력에 한 번도 없었다(삭제면: 상대가 그 경로를 지운 적이 없다)
  ⓑ 대상이 **상대 이력에서 더 새 판본**을 가진 적이 있는데 지금은 더 옛 판본(또는 삭제)을 든다 = 되돌림
판본의 새것/옛것은 상대 브랜치 자기 이력의 위상 순서(`--topo-order`)로 가른다 -- 커밋 시각에 기대지 않는다.
그래도 동기 지점이 없으니 "언제부터" 를 모르는 **약한 판정**이다(예: 대상이 새 판본을 한 번도 받지 않은 채 옛
판본으로 되돌린 것은 ⓑ 로 보이지 않는다). 판정 근거는 `basis` 로 드러난다(`ledger-sync-point` · `since` ·
`blob-ancestry`) -- 대체 판정이 원판정인 척하지 않고, 동기화도 clean 줄에 약한 판정임을 적는다.
"""
from __future__ import annotations

import argparse
import hashlib
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

#: 분류표 빈칸의 자리표시자 머리. **소유자는 이 파일이다** -- 종료 시퀀스의 빈칸 생성기(emit_skeleton)는
#: 이 상수를 적재해 빈칸을 만들고, `validate_entry` 는 같은 상수로 남은 빈칸을 거부한다. 두 자리에 손으로
#: 적으면 한쪽 철자가 바뀌는 순간 "채우지 않은 분류표" 가 원장에 들어간다(2026-09-14 실측: `<FILL>` 이
#: '비어 있지 않음' 으로 통과했다).
FILL_MARK = "<FILL"
#: 자리표시자의 **닫힌 형식**: `<FILL>` · `<FILL: 안내>`. 문자열 어디에 있든 잡되, 앞뒤 꺾쇠가 겹친 형태는 뺀다 --
#: 캠페인 뼈대의 자리표시자 `<<FILL>>` 를 **인용한 근거 산문**이 "채우지 않은 칸" 으로 오인되면 사람이 승인한
#: 분류표를 고쳐야 하고 그 순간 승인이 무효가 된다(2026-09-14 ⑧-pre D2 리뷰: ⑧ 의 campaigns/_template 행이 그 형태다).
FILL_RE = re.compile(r"(?<!<)" + re.escape(FILL_MARK) + r"(?::[^<>]*)?>(?!>)")

#: 착지 탐색에서 대상 브랜치를 거슬러 보는 커밋 수의 상한. 동기화는 수일 간격이고 한 회차 사이 커밋은
#: 수십 건이라 넉넉한 국소 상수다(이 파일에서만 쓴다). 넘으면 착지를 못 찾은 것으로 **보고**한다(`searched`).
MAX_LANDING_SCAN = 400

#: 동기화 스크립트의 bash 배열을 **읽는다**(다시 적지 않는다). 같은 목록이 두 자리에 있으면
#: 갈라지고, 갈라진 쪽이 조용히 늦는다 -- 이 저장소가 그 형태로 네 번 침묵 누락을 냈다.
#: 배포검증이 이미 같은 방식으로 이 스크립트의 배열을 파싱하고 있어 선례도 있다.
_SH_ARRAY_RE = r"^{name}=\((.*?)^\)"

#: `case` 문 docs arm 한 줄 머리(`docs/…|docs/…)`). 본문은 `;;` 까지 이어 읽는다.
_DOCS_ARM_RE = re.compile(r"^\s*((?:docs/[^\s|)]+)(?:\|docs/[^\s|)]+)*)\)(.*)$")

_ZERO_SHA_RE = re.compile(r"^0+$")


class LedgerUnreadable(RuntimeError):
    """원장 파일은 있는데 읽을 수 없다 -- 빈 원장으로 접으면 다음 append 가 이력을 덮어쓴다."""


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


def _docs_case_arms(src: str) -> list[tuple[list[str], str]]:
    """`case` 문에서 `docs/` 로 시작하는 arm 전수 → [(패턴들, 본문)]. 본문은 `;;` 까지."""
    arms: list[tuple[list[str], str]] = []
    lines = src.splitlines()
    i = 0
    while i < len(lines):
        m = _DOCS_ARM_RE.match(lines[i])
        if not m:
            i += 1
            continue
        body = [m.group(2)]
        j = i
        while ";;" not in body[-1] and j + 1 < len(lines):
            j += 1
            body.append(lines[j])
        arms.append((m.group(1).split("|"), "\n".join(body)))
        i = j + 1
    return arms


def common_layer(repo: Path) -> dict:
    """동기화 대상(공통층) pathspec. 소유자는 `sync_branches.sh` 이고 여기서는 읽기만 한다.

    docs 평면은 배열이 아니라 **case arm** 으로 선언돼 있다(2026-09-14 · ⑧-pre D2 S3). 종전 판본은 arm 이
    **있는지만** 정규식으로 보고 `["docs/report", "docs/benchmark"]` 를 손으로 돌려줬고, 게다가 갈라짐 판정의
    pathspec 은 그 값을 쓰지도 않았다 -- `docs/*/example.md` 변경이 divergence 에도 분류표 빈칸에도 보이지
    않았다. 이제 arm 의 **패턴 자체**를 읽고 동기화 의미로 가른다:

      - `include_docs` = 복사 arm ∩ 미러 삭제 arm = **덮어쓰고 지우는** 경로(`docs/*/example.md`). 갈라짐 판정 대상.
      - `union_docs`   = report arm(합집합 수렴) + 복사만 하고 지우지 않는 arm(인증서 · 동기화 주석 "합집합으로
                         수렴"). 대상 전용 발행본이 있는 것이 정상이므로 **판정에서 뺀다**(기존 docs/report 규칙과 같다).

    그래서 pathspec 이 둘이다(2026-09-14 ⑧-pre D2 리뷰 교정):
      - `pathspec`       = **갈라짐 판정** 대상(합집합 수렴분 제외).
      - `reach_pathspec` = **이 동기화가 옮기는 것**(분류표 빈칸의 도달범위). 복사만 하는 arm(인증서)은 대상 쪽 같은
                           경로를 **충돌 검사 없이 덮으므로** 사람이 보는 표에서 빼면 안 된다 -- 종전 빈칸은 `docs/benchmark`
                           를 손으로 붙여 인증서를 보였다. report arm 은 자기 충돌 RED(REPORT_CONFLICT)가 있어 빠진다.
    """
    src = (repo / SYNC_SCRIPT_REL).read_text(encoding="utf-8")
    include = _shell_array(src, "ALLOWLIST")
    arms = _docs_case_arms(src)
    copy_arms = [p for pats, body in arms if 'PATHS+=("$f")' in body and "REPORT_ADD" not in body
                 for p in pats]
    union_arms = [p for pats, body in arms if "REPORT_ADD" in body for p in pats]
    mirror_arms = [p for pats, body in arms if "DELETE_PATHS+=" in body for p in pats]
    if not include or not copy_arms or not union_arms or not mirror_arms:
        # 읽지 못한 것을 빈 목록으로 접으면 판정 대상이 조용히 줄어든다 -- 소리낸다.
        raise RuntimeError(
            f"{SYNC_SCRIPT_REL} 에서 공통층 선언을 읽지 못했다 "
            f"(ALLOWLIST={len(include)} · 복사 arm={copy_arms} · 합집합 arm={union_arms} · 미러 arm={mirror_arms})")
    include_docs = [p for p in copy_arms if p in mirror_arms]
    copy_only_docs = [p for p in copy_arms if p not in mirror_arms]
    union_docs = union_arms + copy_only_docs
    excludes = [tok.strip("'") for tok in _exclude_pathspecs(src)]
    pathspec = include + include_docs + excludes + [":(exclude)" + p for p in union_docs]
    reach_pathspec = include + include_docs + copy_only_docs + excludes + [":(exclude)" + p for p in union_arms]
    return {"include": include, "include_docs": include_docs, "union_docs": union_docs,
            "copy_only_docs": copy_only_docs, "exclude": excludes, "pathspec": pathspec,
            "reach_pathspec": reach_pathspec, "source": SYNC_SCRIPT_REL}


def _diff_pathspec(repo: Path) -> list[str]:
    # docs/report(합집합 수렴)는 **갈라짐 판정에서 뺀다** -- 양쪽에 다른 발행본이 있는 것이 정상이고,
    # 같은 경로의 충돌만 동기화가 따로 RED 로 잡는다. 규칙의 정본은 common_layer 의 union_docs 다.
    return common_layer(repo)["pathspec"]


# ---------------------------------------------------------------- 원장 읽기

def _parse_ledger_text(text: str) -> tuple[dict | None, str]:
    try:
        doc = json.loads(text)
    except ValueError as exc:
        return None, f"JSON 파싱 실패: {exc}"
    if not isinstance(doc, dict) or not isinstance(doc.get("entries", []), list):
        return None, "최상위가 {entries: [...]} 모양이 아니다"
    if any(not isinstance(e, dict) for e in doc.get("entries") or []):
        return None, "entries 에 객체가 아닌 항목이 있다"
    return doc, "ok"


def read_ledgers(repo: Path, ledger_refs: tuple[str, ...] | list[str] = ()) -> dict:
    """워크트리 원장 + `ledger_refs` 각 ref 의 원장을 entry_id 합집합으로 **읽기만** 한다(쓰지 않는다)."""
    sources: list[dict] = []
    merged: dict[str, dict] = {}
    conflicts: list[str] = []

    def take(label: str, text: str | None, status_if_none: str = "absent", detail: str = "") -> None:
        if text is None:
            row = {"source": label, "status": status_if_none, "entries": 0}
            if detail:
                row["detail"] = detail
            sources.append(row)
            return
        doc, why = _parse_ledger_text(text)
        if doc is None:
            sources.append({"source": label, "status": "unreadable", "detail": why})
            return
        ents = doc.get("entries") or []
        sources.append({"source": label, "status": "ok", "entries": len(ents)})
        for e in ents:
            eid = e.get("entry_id")
            if eid in merged and merged[eid] != e:
                if eid not in conflicts:
                    conflicts.append(eid)
                continue
            merged.setdefault(eid, e)

    path = repo / LEDGER_REL
    if path.is_file():
        try:
            take("worktree", path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError) as exc:
            sources.append({"source": "worktree", "status": "unreadable", "detail": str(exc)})
    else:
        take("worktree", None)
    for ref in ledger_refs:
        if _git(repo, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}").returncode != 0:
            take(f"ref:{ref}", None, "ref-unresolved", "ref 를 커밋으로 풀 수 없다")
            continue
        proc = _git(repo, "show", f"{ref}:{LEDGER_REL}")
        take(f"ref:{ref}", proc.stdout if proc.returncode == 0 else None)
    entries = sorted(merged.values(), key=lambda e: (e.get("utc") or "", e.get("entry_id") or ""))
    return {"entries": entries, "sources": sources, "conflicts": conflicts}


# ---------------------------------------------------------------- 동기 지점

def _empty_tree(repo: Path) -> str:
    proc = _git(repo, "hash-object", "-t", "tree", "/dev/null")
    return proc.stdout.strip() if proc.returncode == 0 else "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


def _fingerprint(repo: Path, commit: str, spec: list[str], cache: dict) -> str | None:
    """커밋의 **공통층 트리 지문**. pathspec 해석은 git 이 한다(파이썬으로 다시 적지 않는다 -- 선행 `**/` 선례)."""
    if commit in cache:
        return cache[commit]
    empty = cache.setdefault("__empty_tree__", _empty_tree(repo))
    proc = _git(repo, "diff", "--raw", "-z", "--no-abbrev", "--no-renames", empty, commit, "--", *spec)
    fp = hashlib.sha256(proc.stdout.encode("utf-8", "surrogateescape")).hexdigest() \
        if proc.returncode == 0 else None
    cache[commit] = fp
    return fp


def _find_landing(repo: Path, spec: list[str], departure: str, dep_commit: str, target: str,
                  cache: dict) -> tuple[str, str] | None:
    """항목 하나의 착지 → (착지한 출발 커밋, 대상 커밋) 또는 None.

    출발 계보 = 기록된 출발 커밋 + 그 뒤 출발 브랜치의 ancestry-path. 대상 커밋 중 그 계보의 어느 커밋과
    공통층 트리가 같은 것이 착지 후보다. 착지한 출발 커밋은 같은 지문 중 **가장 새 것**이다(그 뒤 공통층을
    건드리지 않은 커밋은 같은 트리를 든다).

    후보가 여럿이면 **계보상 가장 새 출발 커밋**에 맞는 후보를 고르고, 같으면 가장 새 대상 커밋이다. 대상이 옛
    출발 트리로 되돌린 커밋은 계보상 더 옛 지문에 맞으므로 착지로 뽑히지 않는다(모듈 docstring §동기 지점).
    """
    if _git(repo, "rev-parse", "--verify", "--quiet", f"{dep_commit}^{{commit}}").returncode != 0:
        return None
    line = _git(repo, "rev-list", "--ancestry-path", f"{dep_commit}..{departure}").stdout.split()
    line.append(_git(repo, "rev-parse", f"{dep_commit}^{{commit}}").stdout.strip())
    by_fp: dict[str, tuple[int, str]] = {}              # 지문 → (계보 순위 0=가장 새 것, 출발 커밋)
    for rank, sha in enumerate(line):                   # rev-list 는 새것부터 -- 먼저 본 것이 가장 새 것
        fp = _fingerprint(repo, sha, spec, cache)
        if fp is not None and fp not in by_fp:
            by_fp[fp] = (rank, sha)
    best: tuple[int, str, str] | None = None
    for sha in _git(repo, "rev-list", f"--max-count={MAX_LANDING_SCAN}", target).stdout.split():
        fp = _fingerprint(repo, sha, spec, cache)
        if fp is None or fp not in by_fp:
            continue
        rank, dep_sha = by_fp[fp]
        if best is None or rank < best[0]:
            best = (rank, dep_sha, sha)
            if rank == 0:                               # 계보 맨 앞과 같다 -- 더 새 착지는 없다
                break
    return (best[1], best[2]) if best else None


def sync_point(repo: Path, branch: str, *, counterpart: str | None = None,
               ledger_refs: tuple[str, ...] | list[str] = ()) -> dict:
    """브랜치 `branch` 의 마지막 공통층 동기 지점 커밋.

    원장 항목을 새것부터 훑는다. 항목이 `branch` 로 착지했으면(branch ≠ 출발) 그 대상 커밋이, 항목이 `branch` 에서
    출발했고 상대(`counterpart`)에 착지했으면 **착지한 출발 커밋**이 동기 지점이다. 착지하지 않은 항목(중단된
    회차 · ④ 직후 아직 ⑤ 전인 이번 회차)은 건너뛰고 더 오래된 항목을 본다.

    출발 브랜치인데 `counterpart` 가 없으면 착지를 확인할 수 없어 기록된 출발 커밋을 돌려주되
    `landing_verified: false` 로 표시한다(확인한 척하지 않는다).
    """
    led = read_ledgers(repo, ledger_refs)
    base = {"sources": led["sources"]}
    if led["conflicts"]:
        return {**base, "status": "ledger-conflict", "commit": None, "conflicts": led["conflicts"],
                "detail": "같은 entry_id 가 원장 출처마다 다른 내용이다 -- append-only 가 깨졌다. 어느 쪽도 고르지 않는다"}
    bad = [s for s in led["sources"] if s["status"] in ("unreadable", "ref-unresolved")]
    if bad:
        return {**base, "status": "ledger-unreadable", "commit": None,
                "detail": "원장을 읽지 못했다 -- 빈 원장으로 접지 않는다: "
                          + "; ".join(f"{s['source']}={s['status']}({s.get('detail', '')})" for s in bad)}
    entries = led["entries"]
    if not entries:
        return {**base, "status": "empty-ledger", "commit": None,
                "detail": "원장에 항목이 없다 -- 첫 동기화(부트스트랩)다"}

    spec = _diff_pathspec(repo)
    cache: dict = {}
    searched: list[dict] = []
    for entry in reversed(entries):
        eid, dep, dc = entry.get("entry_id"), entry.get("departure"), entry.get("departure_commit")
        if not dep or not dc:
            searched.append({"entry_id": eid, "result": "출발 필드 결손 -- 건너뜀"})
            continue
        if branch != dep and counterpart is not None and dep != counterpart:
            searched.append({"entry_id": eid, "result": f"다른 브랜치 쌍({dep}) -- 건너뜀"})
            continue
        if branch == dep and counterpart is None:
            return {**base, "status": "departure", "commit": dc, "entry_id": eid, "landing_verified": False,
                    "detail": "출발 브랜치이고 --counterpart 가 없어 착지를 확인하지 않았다 -- 기록된 출발 커밋이다"}
        target = counterpart if branch == dep else branch
        landing = _find_landing(repo, spec, dep, dc, target, cache)
        if landing is None:
            searched.append({"entry_id": eid, "result": f"{target} 에 착지 흔적(공통층 트리 동치) 없음"})
            continue
        landed_dep, landed_target = landing
        # counterpart_commit = 착지 쌍 중 **상대 브랜치 쪽** 커밋. 갈라짐 판정이 "동기 지점 이후 상대 계보" 를 셀 기준이다.
        common = {**base, "entry_id": eid, "recorded_departure_commit": dc,
                  "landed_departure_commit": landed_dep, "landing_commit": landed_target,
                  "counterpart_commit": landed_target if branch == dep else landed_dep,
                  "departure_drift": _fingerprint(repo, dc, spec, cache) != _fingerprint(repo, landed_dep, spec, cache),
                  "landing_verified": True, "skipped": searched}
        if common["departure_drift"]:
            common["drift_note"] = ("원장 기록 커밋과 실제로 내려앉은 출발 트리가 다르다 -- 같은 분류표로 재실행한 "
                                    "회차다(과거 항목은 고치지 않는다 · append-only)")
        if branch == dep:
            return {**common, "status": "departure-landed", "commit": landed_dep}
        return {**common, "status": "converged", "commit": landed_target}
    return {**base, "status": "unresolved", "commit": None, "searched": searched,
            "detail": ("원장 항목 중 어느 것도 이 브랜치 쌍에 내려앉은 흔적(공통층 트리 동치)을 찾지 못했다 -- "
                       "모든 회차가 중단됐거나 이력이 다시 쓰였다")}


# ---------------------------------------------------------------- 갈라짐

def _parse_raw_z(text: str) -> list[tuple[str, str, str]]:
    """`--raw -z` 출력 → [(옛 blob, 새 blob, 경로)]. `--no-renames` 전제(경로 하나)."""
    out: list[tuple[str, str, str]] = []
    tokens = text.split("\0")
    i = 0
    while i < len(tokens):
        tok = tokens[i].lstrip("\n")
        if tok.startswith(":") and i + 1 < len(tokens):
            meta = tok[1:].split()
            if len(meta) >= 5:
                out.append((meta[2], meta[3], tokens[i + 1]))
            i += 2
            continue
        i += 1
    return out


def _history(repo: Path, rev: str, paths: list[str]) -> dict | None:
    """`rev`(ref 또는 `a..b` 범위) 이력의 경로별 blob 흔적 → {경로: {...}} 또는 None(git 실패).

    `new` = 그 이력의 커밋이 **들여온** 바이트 → 가장 새로 들여온 위상 순위(0=가장 새 커밋) · `old` = 바뀌기 전
    바이트 집합 · `deleted` = 그 경로를 가장 새로 지운 위상 순위(없으면 None). 한 번의 `git log --topo-order` 로 모은다.
    순위는 **그 이력 안에서만** 뜻이 있다(두 브랜치의 순위를 서로 비교하지 않는다).
    """
    if not paths:
        return {}
    proc = _git(repo, "log", "--topo-order", "--format=%x01", "--raw", "-z", "--no-abbrev", "--no-renames", "-m",
                rev, "--", *[":(literal)" + p for p in paths])
    if proc.returncode != 0:
        return None
    out: dict[str, dict] = {}
    idx = -1
    tokens = proc.stdout.split("\0")
    i = 0
    while i < len(tokens):
        tok = tokens[i].lstrip("\n")
        if tok.startswith("\x01"):
            idx += 1
            i += 1
            continue
        if tok.startswith(":") and i + 1 < len(tokens):
            meta = tok[1:].split()
            if len(meta) >= 5:
                old, new, path = meta[2], meta[3], tokens[i + 1]
                h = out.setdefault(path, {"new": {}, "old": set(), "deleted": None})
                if _ZERO_SHA_RE.match(new):
                    if h["deleted"] is None:
                        h["deleted"] = idx
                else:
                    h["new"].setdefault(new, idx)          # 새것부터 읽으므로 처음 본 순위가 가장 새 것
                if not _ZERO_SHA_RE.match(old):
                    h["old"].add(old)
            i += 2
            continue
        i += 1
    return out


def _pending(repo: Path, counterpart: str, branch: str, spec: list[str]) -> dict[str, str] | None:
    """동기화가 바꿀 공통층 경로 → 대상 쪽 blob(대상에 없으면 0…). git 실패는 None."""
    diff = _git(repo, "diff", "--raw", "-z", "--no-abbrev", "--no-renames", counterpart, branch, "--", *spec)
    if diff.returncode != 0:
        return None
    return {path: new for _old, new, path in _parse_raw_z(diff.stdout)}


def _unjudged(basis: str, sp: dict | None, detail: str) -> dict:
    return {"status": "unjudged", "diverged": None, "basis": basis, "base": None, "files": [], "commits": [],
            "sync_point": sp, "detail": detail}


def blob_ancestry(repo: Path, branch: str, counterpart: str, spec: list[str], sp: dict | None) -> dict:
    """동기 지점 없이 판정한다(모듈 docstring §동기 지점이 없을 때의 ⓐ·ⓑ). 삭제·되돌림도 본다."""
    pending = _pending(repo, counterpart, branch, spec)
    if pending is None:
        return _unjudged("blob-ancestry", sp, f"git diff {counterpart} {branch} 실패")
    paths = sorted(pending)
    cp_hist = _history(repo, counterpart, paths)
    tg_hist = _history(repo, branch, paths)
    if cp_hist is None or tg_hist is None:
        return _unjudged("blob-ancestry", sp, "git log 실패(경로 이력을 읽지 못했다)")
    target_only: list[str] = []
    reasons: dict[str, str] = {}
    for path in paths:
        blob = pending[path]
        th, ch = tg_hist.get(path), cp_hist.get(path)
        if not th:
            continue                                    # 대상은 이 경로를 가진 적이 없다 = 상대의 추가(정상 전파)
        deleted = bool(_ZERO_SHA_RE.match(blob))
        # 대상 현재 판본의 상대 이력 순위(0=가장 새 것). 상대 이력에 없으면 None = ⓐ
        if deleted:
            here = ch["deleted"] if ch else None
        else:
            here = ch["new"].get(blob) if ch else None
            if here is None and ch and blob in ch["old"]:
                here = max(ch["new"].values(), default=0) + 1   # 들여온 커밋이 이력 밖(얕은 경계)이면 가장 옛 것으로 본다
        held = set(th["new"]) | th["old"]               # 대상이 한 번이라도 든 판본
        newer_held = sorted(b for b in held if here is not None and ch and ch["new"].get(b, here) < here)
        if here is None or newer_held:
            target_only.append(path)
            reasons[path] = ("대상 삭제" if deleted else "대상 바이트") + (
                " · 상대 이력에 없음" if here is None
                else f" · 상대 이력의 더 새 판본({newer_held[0][:12]})을 든 뒤 옛 판본으로 되돌림")
    res = {"status": "diverged" if target_only else "clean", "diverged": bool(target_only),
           "basis": "blob-ancestry", "base": None, "files": target_only, "commits": [], "sync_point": sp,
           "file_reasons": reasons,
           "detail": (f"동기 지점이 없어({(sp or {}).get('status')}) blob 조상으로 판정했다 -- {branch} 의 공통층 중 "
                      f"{counterpart} 과 다른 {len(paths)}건 · 동기 지점이 없어 '언제부터' 를 모르는 약한 판정이다")}
    if target_only:
        res["remediation"] = (f"{branch} 의 위 파일 변경은 {counterpart} 가 받지 않았다 -- 덮으면 사라진다. 보존할 변경이면 "
                              f"{counterpart} 로 먼저 옮기고(cherry-pick), 버릴 변경이면 사람이 {branch} 에서 되돌린 뒤 "
                              "다시 동기화하라.")
    return res


def divergence(repo: Path, branch: str, since: str | None = None, *, counterpart: str | None = None,
               ledger_refs: tuple[str, ...] | list[str] = ()) -> dict:
    """동기 지점 이후 `branch` 의 공통층이 갈라졌는가. 갈라졌으면 파일과 커밋 목록을 함께 준다.

    결과 status: clean · diverged · unjudged(판정 불가 -- 원장 무결성 · git 실패) · no-sync-point(상대 없음).
    """
    sp = None
    base = since
    basis = "since" if since else "ledger-sync-point"
    if not base:
        sp = sync_point(repo, branch, counterpart=counterpart, ledger_refs=ledger_refs)
        if sp.get("status") in ("ledger-conflict", "ledger-unreadable"):
            # 원장이 깨진 것은 "동기 지점 없음" 이 아니다 -- 약한 대체 판정으로 접지 않는다(모듈 docstring).
            return _unjudged("ledger-integrity", sp,
                             f"원장 무결성 결함({sp['status']}) -- 동기 지점을 정할 수 없어 판정하지 않는다. "
                             "원장을 고친 뒤(append-only 복원 · ref 확인) 다시 판정하라")
        base = sp.get("commit")
    spec = _diff_pathspec(repo)
    if not base:
        if counterpart is None:
            return {"status": "no-sync-point", "diverged": None, "sync_point": sp,
                    "detail": ("동기 지점이 없고 --counterpart 가 없어 blob 조상 판정도 할 수 없다 "
                               "(첫 실행이거나 중단된 동기화)")}
        return blob_ancestry(repo, branch, counterpart, spec, sp)
    quiet = _git(repo, "diff", "--quiet", base, branch, "--", *spec)
    if quiet.returncode not in (0, 1):
        return _unjudged(basis, sp, f"git diff {base} {branch} 실패: {quiet.stderr.strip()[:300]}")
    if quiet.returncode == 0:
        return {"status": "clean", "diverged": False, "basis": basis, "base": base, "files": [],
                "commits": [], "absorbed": [], "sync_point": sp}
    names = _git(repo, "diff", "--name-only", "-z", base, branch, "--", *spec)
    changed = [f for f in names.stdout.split("\0") if f]
    absorbed: list[dict] = []
    if counterpart is None:
        files = changed
    else:
        pending = _pending(repo, counterpart, branch, spec)
        if pending is None:
            return _unjudged(basis, sp, f"git diff {counterpart} {branch} 실패")
        cp_base = (sp or {}).get("counterpart_commit")
        cand = {p: pending[p] for p in changed if p in pending}
        carried: set[str] = set()
        if cand and cp_base:
            hist = _history(repo, f"{cp_base}..{counterpart}", sorted(cand))
            if hist is None:
                return _unjudged(basis, sp, f"git log {cp_base}..{counterpart} 실패")
            for p, blob in cand.items():
                h = hist.get(p)
                if h and ((h["deleted"] if _ZERO_SHA_RE.match(blob) else blob in h["new"])):
                    carried.add(p)
        files = [p for p in changed if p in cand and p not in carried]
        absorbed = [{"file": p, "why": ("상대와 바이트 동일 -- 동기화가 바꿀 것이 없다" if p not in cand else
                                        "동기 지점 이후 상대 계보에 같은 바이트가 들어왔다(옮겨졌다)")}
                    for p in changed if p not in files]
    if not files:
        return {"status": "clean", "diverged": False, "basis": basis, "base": base, "files": [],
                "commits": [], "absorbed": absorbed, "sync_point": sp}
    commits = _git(repo, "log", "--oneline", f"{base}..{branch}", "--",
                   *[":(literal)" + f for f in files]).stdout.splitlines()
    return {"status": "diverged", "diverged": True, "basis": basis, "base": base,
            "files": files, "commits": commits, "absorbed": absorbed, "sync_point": sp,
            "remediation": ("대상 브랜치의 공통층이 출발 브랜치가 받지 않은 변경을 들고 있다. 덮어쓰면 사라진다 -- "
                            "보존할 변경이면 위 커밋을 출발 브랜치로 먼저 옮기고(cherry-pick · 옮기면 이 판정이 흡수로 "
                            "인식한다), 버릴 변경이면 사람이 대상 브랜치에서 되돌린 뒤 다시 동기화하라.")}


# ---------------------------------------------------------------- 원장 쓰기

def load(repo: Path) -> dict:
    """쓰기 경로용 적재. 파일이 없으면 빈 원장, **있는데 못 읽으면 예외**다.

    종전 판본은 못 읽는 원장을 빈 원장으로 접었고, 그 위의 append 가 `save` 로 **파일 전체를 새 항목 하나로
    덮어썼다** -- append-only 를 지키려던 함수가 이력을 지우는 경로였다(2026-09-14 ⑧-pre D2 리뷰 발견).
    """
    path = repo / LEDGER_REL
    if not path.is_file():
        return {"schema_version": SCHEMA_VERSION, "entries": []}
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise LedgerUnreadable(f"{LEDGER_REL}: {exc}") from exc
    doc, why = _parse_ledger_text(text)
    if doc is None:
        raise LedgerUnreadable(f"{LEDGER_REL}: {why}")
    return doc


def save(repo: Path, doc: dict) -> Path:
    path = repo / LEDGER_REL
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def _placeholder_paths(obj, where: str = "") -> list[str]:
    """자리표시자(`FILL_RE` 의 닫힌 형식)가 남은 문자열 값의 위치 전수(키 경로)."""
    if isinstance(obj, str):
        return [where or "<root>"] if FILL_RE.search(obj) else []
    if isinstance(obj, dict):
        return [p for k, v in obj.items() for p in _placeholder_paths(v, f"{where}.{k}" if where else str(k))]
    if isinstance(obj, list):
        return [p for i, v in enumerate(obj) for p in _placeholder_paths(v, f"{where}[{i}]")]
    return []


def validate_entry(entry: dict) -> list[str]:
    problems = []
    if not isinstance(entry, dict):
        return [f"항목이 객체가 아니다: {type(entry).__name__}"]
    for key in ("entry_id", "utc", "departure", "departure_commit", "approved_by",
                "approved_utc", "classification"):
        if not entry.get(key):
            problems.append(f"필수 필드 없음: {key}")
    # 빈칸 생성기가 넣은 자리표시자는 '비어 있지 않음' 을 통과한다 -- 그래서 따로 본다(⑧-pre D2 S4).
    for where in _placeholder_paths(entry):
        problems.append(f"자리표시자가 남아 있다({FILL_MARK}…>): {where} -- ③ 승인 전에 채워야 한다")
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
    try:
        doc = load(repo)
    except LedgerUnreadable as exc:
        return {"status": "rejected", "problems": [f"기존 원장을 읽을 수 없어 쓰지 않는다(덮어쓰기 방지): {exc}"]}
    entries = doc.get("entries") or []
    for prior in entries:
        if prior.get("entry_id") != entry["entry_id"]:
            continue
        # 같은 id 가 **같은 내용**으로 이미 있으면 재기록은 no-op 이다(멱등).
        #
        # 왜: 종료 시퀀스는 ④ 원장 -> ⑤ 동기화 -> ⑥ 서브 순서인데, ④ 가 성공하고 ⑤ 가 RED 로
        # 멈추는 일이 실제로 일어난다(2026-09-12 첫 실행: DIRTY_WORKTREE). 그때 재실행하면 ④ 가
        # 자기가 방금 적은 항목을 보고 거부했다 -- 즉 **append-only 를 지키려던 규칙이 사람을
        # 원장 손편집으로 몰았다**. 되돌릴 수 없는 규칙은 우회를 만든다.
        #
        # 내용이 **다르면** 여전히 거부한다. 그것은 재실행이 아니라 이력 덮어쓰기이고, 이 규칙이
        # 실제로 막아야 하는 것은 그쪽이다. (재실행 회차가 실제로 내려앉힌 트리는 동기 지점 탐색이
        # 트리 동치로 찾는다 -- 원장을 고치지 않는다.)
        if prior == entry:
            return {"status": "already-recorded", "entry_id": entry["entry_id"],
                    "total": len(entries), "path": LEDGER_REL}
        return {"status": "rejected",
                "problems": [f"entry_id 중복이고 내용이 다르다: {entry['entry_id']} -- "
                             "원장은 append-only 다(재실행이면 같은 바이트여야 한다)"]}
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
    other, why = _parse_ledger_text(proc.stdout)
    if other is None:
        return {"status": "unreadable", "detail": why, "added": 0}
    try:
        doc = load(repo)
    except LedgerUnreadable as exc:
        return {"status": "unreadable", "detail": f"기존 원장을 읽을 수 없어 쓰지 않는다: {exc}", "added": 0}
    have = {e.get("entry_id") for e in doc.get("entries") or []}
    added = [e for e in (other.get("entries") or []) if e.get("entry_id") not in have]
    if added:
        doc.setdefault("entries", []).extend(added)
        doc["entries"].sort(key=lambda e: (e.get("utc") or "", e.get("entry_id") or ""))
        doc["schema_version"] = SCHEMA_VERSION
        save(repo, doc)
    return {"status": "merged", "added": len(added),
            "added_ids": [e.get("entry_id") for e in added], "total": len(doc.get("entries") or [])}


# ---------------------------------------------------------------- self-test

def _self_test() -> int:
    import os
    import shutil
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

    # ── S4: 빈칸 생성기의 자리표시자는 '비어 있지 않음' 이지만 채워진 것이 아니다 ──────────────────
    for key in ("entry_id", "utc", "departure_commit", "approved_by", "approved_utc", "departure"):
        bad = dict(good, **{key: f"{FILL_MARK}: 채워라>"})
        probs = validate_entry(bad)
        ck(f"★S4 음성: {key} 에 자리표시자가 남으면 거부", any(key in p and FILL_MARK in p for p in probs), probs)
    bad_row = dict(good, classification=[dict(good["classification"][0], reason=f"{FILL_MARK}>")])
    ck("★S4 음성: 분류 행 근거 자리표시자 거부",
       any("classification[0].reason" in p for p in validate_entry(bad_row)), validate_entry(bad_row))
    skeleton = {"entry_id": f"{FILL_MARK}: 예 sync-YYYYMMDDHHMM>", "utc": f"{FILL_MARK}: 시각>",
                "departure": "multi-node", "departure_commit": f"{FILL_MARK}: HEAD>",
                "approved_by": f"{FILL_MARK}: 사람>", "approved_utc": f"{FILL_MARK}>",
                "classification": [{"file": "CLAUDE.md", "verdict": "<common_promote|branch_only>",
                                    "reason": f"{FILL_MARK}>", "provenance": "agent-judged"}]}
    quoting = dict(good, classification=[dict(good["classification"][0],
                                              reason="provenance 기본값을 <<FILL>> 빈칸으로 바꿔 precheck 가 막는다")])
    ck("★S4 양성: 캠페인 뼈대 자리표시자 <<FILL>> 을 **인용한** 근거는 채워진 칸이다(승인 뒤 표를 고치게 하지 않는다)",
       validate_entry(quoting) == [], validate_entry(quoting))
    partial = dict(good, classification=[dict(good["classification"][0], reason=f"근거 일부만 적었다 {FILL_MARK}>")])
    ck("★S4 음성: 산문 뒤에 남은 자리표시자도 거부",
       any("classification[0].reason" in p for p in validate_entry(partial)), validate_entry(partial))
    sk_probs = validate_entry(skeleton)
    ck("★S4 음성: 빈칸 그대로의 분류표는 자리표시자 6건 + 판정 값역을 전부 드러낸다",
       sum(1 for p in sk_probs if FILL_MARK in p) == 6 and any("판정은" in p for p in sk_probs), sk_probs)

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        (root / ".claude/policies").mkdir(parents=True)
        ck("첫 append", append(root, good)["status"] == "appended")
        # 같은 바이트의 재실행은 통과(멱등) · 내용이 갈린 같은 id 는 거부(이력 덮어쓰기)
        ck("동일 재append = 멱등", append(root, good)["status"] == "already-recorded",
           append(root, good))
        drifted = dict(good, departure_commit="cafef00")
        ck("내용 다른 중복 = 거부", append(root, drifted)["status"] == "rejected",
           append(root, drifted))
        ck("적재", len(load(root)["entries"]) == 1)
        ck("★S4 음성: 자리표시자 항목은 원장에 들어가지 않는다",
           append(root, skeleton)["status"] == "rejected" and len(load(root)["entries"]) == 1)
        # 못 읽는 원장 위에 append 하면 이력을 덮어쓰던 경로 -- 이제 쓰지 않는다
        broken = b'{"entries": [ {"entry_id": "old"'
        (root / LEDGER_REL).write_bytes(broken)
        res = append(root, dict(good, entry_id="e2"))
        ck("★음성: 못 읽는 원장 위 append 는 거부하고 바이트를 건드리지 않는다",
           res["status"] == "rejected" and (root / LEDGER_REL).read_bytes() == broken, res)

    # ── 공통층 spec: sync_branches.sh 의 case arm 에서 읽는다(S3) ─────────────────────────────────
    real_sync = Path(__file__).resolve().parent / "sync_branches.sh"
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        (root / SYNC_SCRIPT_REL).parent.mkdir(parents=True)
        shutil.copy2(real_sync, root / SYNC_SCRIPT_REL)
        spec = common_layer(root)
        ck("★S3 docs 판정 대상 = 복사∩미러(docs/*/example.md)", spec["include_docs"] == ["docs/*/example.md"], spec)
        ck("★S3 합집합 수렴(report·인증서)은 판정에서 뺀다",
           "docs/report/*" in spec["union_docs"] and ":(exclude)docs/report/*" in spec["pathspec"], spec)
        ck("★S3 pathspec 에 docs/*/example.md 가 실린다", "docs/*/example.md" in spec["pathspec"], spec)
        ck("★S3 도달범위(분류표 빈칸)는 충돌 검사 없이 덮는 복사 전용 arm(인증서)을 보인다 · report 는 뺀다",
           spec["copy_only_docs"] and all(p in spec["reach_pathspec"] for p in spec["copy_only_docs"])
           and ":(exclude)docs/report/*" in spec["reach_pathspec"]
           and not any(p in spec["pathspec"] for p in spec["copy_only_docs"]), spec)
        (root / SYNC_SCRIPT_REL).write_text("ALLOWLIST=(\n    CLAUDE.md\n)\n", encoding="utf-8")
        try:
            common_layer(root)
            ck("★음성: arm 을 못 읽으면 빈 목록으로 접지 않고 소리낸다", False)
        except RuntimeError:
            pass

    # ── git 픽스처: 착지 탐색(S2) · ref 원장(S1) · blob 조상 · 음성대조 ──────────────────────────────
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
    # 훅 안에서 불려도 실 저장소 인덱스·객체를 건드리지 않게 git 위치 변수를 걷는다(git 이 훅에 넣어 주는 변수들).
    _git_location_vars = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                          "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX")
    for _k in _git_location_vars:
        env.pop(_k, None)

    with tempfile.TemporaryDirectory() as td:
        env["GIT_CONFIG_GLOBAL"] = str(Path(td) / "gitconfig")
        Path(env["GIT_CONFIG_GLOBAL"]).write_text("[commit]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n",
                                                  encoding="utf-8")
        repo = Path(td) / "repo"

        def g(*args: str, cwd: Path | None = None) -> str:
            proc = subprocess.run(["git", "-C", str(cwd or repo), *args], capture_output=True, text=True,
                                  env=env, timeout=60)
            if proc.returncode != 0:
                raise RuntimeError(f"fixture git {args} failed: {proc.stderr.strip()[:300]}")
            return proc.stdout.strip()

        def write(rel: str, text: str, where: Path | None = None) -> None:
            p = (where or repo) / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")

        def commit(msg: str, where: Path | None = None) -> str:
            g("add", "-A", cwd=where)
            g("commit", "-qm", msg, cwd=where)
            return g("rev-parse", "HEAD", cwd=where)

        old_env = dict(os.environ)
        os.environ.update({k: env[k] for k in ("GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM")})
        for _k in _git_location_vars:
            os.environ.pop(_k, None)
        try:
            repo.mkdir()
            g("init", "-q", "-b", "multi-node")
            (repo / SYNC_SCRIPT_REL).parent.mkdir(parents=True)
            shutil.copy2(real_sync, repo / SYNC_SCRIPT_REL)
            write("CLAUDE.md", "v1\n")
            write(".claude/rules/common.md", "c1\n")
            write(".claude/rules/strategy.topology.md", "**topology: multi**\n")
            write("docs/plan/example.md", "p1\n")
            write("docs/report/r1.md", "r1\n")
            commit("A")
            g("branch", "single-node")
            wt = Path(td) / "wt-single"
            g("worktree", "add", "-q", str(wt), "single-node")
            write(".claude/rules/strategy.topology.md", "**topology: single**\n", wt)
            commit("S0 특화층", wt)

            write("CLAUDE.md", "v2\n")
            b = commit("B")
            e1 = dict(good, entry_id="sync-1", utc="2026-09-12T06:16:53Z", departure="multi-node",
                      departure_commit=b)
            ck("원장 E1 append", append(repo, e1)["status"] == "appended")
            commit("ledger(sync-1)")
            write(".claude/rules/common.md", "c2\n")        # ⑤ RED 뒤 교정 커밋 -- 같은 분류표로 재실행한다
            b2 = commit("B2 교정")
            # 착지: 재실행이 내려앉힌 것은 B 가 아니라 B2 의 트리다(2026-09-12 형태)
            g("checkout", "multi-node", "--", "CLAUDE.md", ".claude/rules/common.md", cwd=wt)
            landing = commit("sync: 공통층 multi-node→single-node", wt)

            spec = _diff_pathspec(repo)
            ck("픽스처가 사고의 형태다: 기록된 출발 커밋 B 와 착지 트리가 다르다",
               subprocess.run(["git", "-C", str(repo), "diff", "--quiet", b, landing, "--", *spec],
                              env=env).returncode != 0)

            # S1 음성대조: 대상 워크트리에는 원장이 없다 -- ref 를 주지 않으면 판정 불가(결함 재현)
            no_ref = divergence(wt, "single-node")
            ck("★S1 음성대조: 대상 워크트리 원장만 보면 no-sync-point(empty-ledger)",
               no_ref["status"] == "no-sync-point" and no_ref["sync_point"]["status"] == "empty-ledger", no_ref)
            sp = sync_point(wt, "single-node", ledger_refs=["multi-node"])
            ck("★S2 착지 = 트리 동치(재실행 회차) · converged · 착지한 출발 커밋 = B2 · drift 표시",
               sp["status"] == "converged" and sp["commit"] == landing and sp["landed_departure_commit"] == b2
               and sp["departure_drift"] is True and sp["recorded_departure_commit"] == b, sp)
            dv = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★S1 ref 원장으로 실제 판정: clean · basis=ledger-sync-point",
               dv["status"] == "clean" and dv["basis"] == "ledger-sync-point" and dv["base"] == landing, dv)
            dep = sync_point(repo, "multi-node", counterpart="single-node")
            ck("★S2 출발 측 동기 지점 = 착지한 출발 커밋(B2 · 기록 B 아님)",
               dep["status"] == "departure-landed" and dep["commit"] == b2, dep)
            legacy = sync_point(repo, "multi-node")
            ck("상대 없이 출발 측을 물으면 기록 커밋 · landing_verified=false 로 표시",
               legacy["status"] == "departure" and legacy["commit"] == b and legacy["landing_verified"] is False,
               legacy)

            # ④ 가 이번 항목을 먼저 적은 뒤 ⑤ 가 판정한다 -- 아직 착지 안 한 새 항목은 건너뛰고 E1 을 본다
            write("CLAUDE.md", "v3\n")
            c = commit("C")
            e2 = dict(good, entry_id="sync-2", utc="2026-09-14T00:00:00Z", departure="multi-node",
                      departure_commit=c)
            ck("원장 E2 append", append(repo, e2)["status"] == "appended")
            commit("ledger(sync-2)")
            sp2 = sync_point(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★S2 새것부터 거꾸로: 착지 전인 이번 항목(E2)을 건너뛰고 E1 착지를 찾는다",
               sp2["status"] == "converged" and sp2["entry_id"] == "sync-1" and sp2["commit"] == landing
               and any(s.get("entry_id") == "sync-2" for s in sp2["skipped"]), sp2)

            # S3: 대상 전용 docs 스켈레톤 변경은 갈라짐으로 보인다 · report 는 합집합이라 안 보인다
            write("docs/report/r2.md", "single 전용 발행본\n", wt)
            commit("report(single)", wt)
            dv_r = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★S3 음성대조: 대상 전용 report 는 갈라짐이 아니다(합집합)", dv_r["status"] == "clean", dv_r)
            write("docs/plan/example.md", "single 전용 스켈레톤 수정\n", wt)
            commit("docs skeleton(single)", wt)
            dv_d = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★S3 대상 전용 docs/*/example.md 변경이 갈라짐으로 보인다",
               dv_d["status"] == "diverged" and dv_d["files"] == ["docs/plan/example.md"], dv_d)
            g("revert", "--no-edit", "HEAD", cwd=wt)
            write(".claude/rules/common.md", "single 전용\n", wt)
            commit("target-only(single)", wt)
            dv_x = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            # base 는 착지 커밋이거나 그 뒤 **출발 트리와 같아진** 대상 커밋(여기서는 revert)이다 -- 둘 다 잃을 것이 없는 자리다.
            ck("★S1 대상 전용 공통층 변경 → diverged (동기 지점 이후 · 파일 지목)",
               dv_x["status"] == "diverged" and dv_x["sync_point"]["status"] == "converged"
               and dv_x["files"] == [".claude/rules/common.md"], dv_x)

            # ★ 교착 해소(⑧-pre D2 리뷰 blocking): 처방("출발 브랜치로 옮긴 뒤 다시")을 따르면 **풀려야** 한다.
            #   종전 판정은 동기 지점 이후 대상 diff 만 봐서, 출발 브랜치가 다른 커밋을 더 한 뒤 같은 바이트를 받아도
            #   영원히 diverged 였다(바이트 동일인데 멈춤). 위 dv_x 가 음성대조다(옮기기 전 = diverged).
            write("CLAUDE.md", "v4\n")
            commit("D 출발 전용")
            write(".claude/rules/common.md", "single 전용\n")
            commit("cherry-pick(single 전용 → multi)")
            dv_c = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★교착 해소: 대상 전용 변경을 출발 브랜치로 옮기면 clean · 흡수(absorbed)로 드러난다",
               dv_c["status"] == "clean" and [a["file"] for a in dv_c["absorbed"]] == [".claude/rules/common.md"], dv_c)
            write(".claude/rules/common.md", "multi 가 옮긴 뒤 더 고쳤다\n")
            commit("E 옮긴 뒤 출발 수정")
            dv_c2 = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★흡수: 옮겨진 뒤 출발 브랜치가 더 고쳐도 clean(대상 바이트가 동기 지점 이후 상대 계보에 있다)",
               dv_c2["status"] == "clean" and [a["file"] for a in dv_c2["absorbed"]] == [".claude/rules/common.md"], dv_c2)
            nocp = divergence(wt, "single-node", ledger_refs=["multi-node"])
            ck("상대 없이 물으면 흡수를 셀 수 없다 -- 동기 지점 이후 대상 변경 전부가 diverged(종전 규칙)",
               nocp["status"] == "diverged" and ".claude/rules/common.md" in nocp["files"], nocp)

            # ★ 되돌림은 흡수가 아니다 · 옛 출발 트리로 되돌린 대상 커밋은 착지가 아니다(⑧-pre D2 리뷰 교정)
            #   landing 에서 갈라진 가지가 common.md 를 c1(= B 의 바이트 · 상대 이력에는 **동기 지점 이전**에만 있다)로
            #   되돌리면 가지의 공통층 트리가 B 와 같아진다. "가장 새 대상 커밋이 계보 어딘가와 같으면 착지" 규칙은 그
            #   되돌림을 착지로 뽑아 clean 을 냈다. 계보상 가장 새 출발 커밋(B2)에 맞는 landing 이 착지다.
            wr = Path(td) / "wt-revert"
            g("worktree", "add", "-q", "-b", "revert-probe", str(wr), landing)
            write(".claude/rules/common.md", "c1\n", wr)
            commit("대상 되돌림(동기 지점 이전 바이트)", wr)
            rv = divergence(wr, "revert-probe", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★되돌림: 옛 출발 트리와 같아진 대상 커밋을 착지로 뽑지 않는다(착지 = landing) · diverged",
               rv["status"] == "diverged" and rv["base"] == landing and rv["files"] == [".claude/rules/common.md"]
               and rv["sync_point"]["landed_departure_commit"] == b2, rv)

            # 원장 무결성: 같은 id 다른 내용 → 합치지 않는다 · 풀리지 않는 ref → 빈 원장으로 접지 않는다
            forged = json.loads((repo / LEDGER_REL).read_text(encoding="utf-8"))
            forged["entries"][0]["approved_by"] = "someone-else"
            write(LEDGER_REL, json.dumps(forged, ensure_ascii=False), wt)
            conf = sync_point(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★음성: 출처마다 다른 같은 entry_id → ledger-conflict", conf["status"] == "ledger-conflict", conf)
            # 갈라짐 판정이 그 상태를 **약한 대체 판정(blob 조상)으로 접으면** 원장이 말하는 갈라짐이 clean 으로 덮인다
            #   (⑧-pre D2 리뷰 major: 이 상태에서 blob 조상은 clean 을 냈다 -- wt 는 지금 대상 전용 되돌림을 들지 않지만
            #   판정 자체가 원장 무결성에서 멈춰야 한다).
            dconf = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★음성: 원장 충돌이면 갈라짐 판정은 unjudged(basis=ledger-integrity) -- blob 조상으로 접지 않는다",
               dconf["status"] == "unjudged" and dconf["basis"] == "ledger-integrity" and dconf["diverged"] is None, dconf)
            (wt / LEDGER_REL).unlink()
            unres = sync_point(wt, "single-node", counterpart="multi-node", ledger_refs=["no-such-ref"])
            ck("★음성: 풀리지 않는 ledger-ref → ledger-unreadable(빈 원장으로 접지 않는다)",
               unres["status"] == "ledger-unreadable", unres)
            dunres = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["no-such-ref"])
            ck("★음성: 못 읽는 원장이면 갈라짐 판정은 unjudged(basis=ledger-integrity)",
               dunres["status"] == "unjudged" and dunres["basis"] == "ledger-integrity", dunres)
            write(LEDGER_REL, '{"entries": [ {"entry_id"', wt)
            dbroken = divergence(wt, "single-node", counterpart="multi-node", ledger_refs=["multi-node"])
            ck("★음성: 깨진 JSON 원장이면 갈라짐 판정은 unjudged", dbroken["status"] == "unjudged", dbroken)
            (wt / LEDGER_REL).unlink()

            # blob 조상: 착지 없는 원장(모든 회차 중단) → 그래도 판정한다
            wb = Path(td) / "wt-blob"
            g("worktree", "add", "-q", "-b", "blob-probe", str(wb), b)
            ghost = dict(good, entry_id="sync-ghost", utc="2026-09-14T01:00:00Z", departure="multi-node",
                         departure_commit=c)
            write(LEDGER_REL, json.dumps({"schema_version": 1, "entries": [ghost]}, ensure_ascii=False), wb)
            # ghost 의 출발 계보(C 이후)에 blob-probe(=B 트리) 와 같은 트리가 없다 → 착지 없음
            nb = divergence(wb, "blob-probe", counterpart="multi-node")
            ck("★blob 조상: 착지 없음 → basis=blob-ancestry · 대상 바이트가 출발 이력에 있으면 clean",
               nb["basis"] == "blob-ancestry" and nb["status"] == "clean"
               and nb["sync_point"]["status"] == "unresolved", nb)
            write(".claude/rules/common.md", "어디에도 없던 바이트\n", wb)
            commit("blob-only", wb)
            nb2 = divergence(wb, "blob-probe", counterpart="multi-node")
            ck("★blob 조상 음성대조: 출발 이력에 한 번도 없던 바이트 → diverged",
               nb2["basis"] == "blob-ancestry" and nb2["status"] == "diverged"
               and nb2["files"] == [".claude/rules/common.md"], nb2)
            nb3 = divergence(wb, "blob-probe")
            ck("상대가 없으면 blob 조상도 못 한다 -- no-sync-point 로 드러낸다", nb3["status"] == "no-sync-point", nb3)

            # blob 조상의 사각(⑧-pre D2 리뷰 major): 삭제는 새 blob 이 0 이라 후보에서 빠졌고, 옛 상대 바이트로의
            #   되돌림은 "상대 이력에 있었다" 로 clean 이었다. 규칙 입력은 상대 이력의 위상 순서다(커밋 시각 ✗) --
            #   이 픽스처는 모든 커밋이 같은 초에 찍히므로 시각 규칙이었다면 여기서 드러난다.
            wd = Path(td) / "wt-del"
            g("worktree", "add", "-q", "-b", "del-probe", str(wd), b)
            g("rm", "-q", ".claude/rules/common.md", cwd=wd)
            commit("대상 전용 삭제", wd)
            write(".claude/rules/new.md", "상대만 새로 만든 파일\n")
            commit("상대 추가")
            nd = divergence(wd, "del-probe", counterpart="multi-node")
            ck("★blob 조상: 대상 전용 삭제 → diverged · 상대만 추가한 파일(대상이 가진 적 없음)은 갈라짐이 아니다",
               nd["basis"] == "blob-ancestry" and nd["status"] == "diverged"
               and nd["files"] == [".claude/rules/common.md"], nd)

            wv = Path(td) / "wt-rev"
            g("worktree", "add", "-q", "-b", "rev-probe", str(wv), b)   # B = CLAUDE.md v2 를 든 가지
            write("CLAUDE.md", "v1\n", wv)                 # A 의 바이트 = 상대 이력에 **있다**(v2 보다 옛 판본)
            commit("대상 되돌림(옛 상대 바이트)", wv)
            nv = divergence(wv, "rev-probe", counterpart="multi-node")
            ck("★blob 조상: 상대 이력의 더 새 판본(v2)을 든 뒤 옛 판본(v1)으로 되돌림 → diverged(file_reasons 표시)",
               nv["status"] == "diverged" and nv["files"] == ["CLAUDE.md"]
               and "되돌림" in nv["file_reasons"]["CLAUDE.md"], nv)
            wf = Path(td) / "wt-fwd"
            first = g("rev-list", "--max-parents=0", "multi-node")
            g("worktree", "add", "-q", "-b", "fwd-probe", str(wf), first)   # A = CLAUDE.md v1 만 든 가지
            write("CLAUDE.md", "v2\n", wf)                 # 상대의 옛 판본으로 **앞으로** 옮겼다(상대는 그 뒤 더 고쳤다)
            commit("대상이 상대의 옛 판본을 뒤늦게 받음", wf)
            nf = divergence(wf, "fwd-probe", counterpart="multi-node")
            ck("★blob 조상 음성대조: 더 새 판본을 든 적 없이 상대의 옛 판본으로 앞으로 옮긴 것은 갈라짐이 아니다",
               nf["status"] == "clean" and nf["basis"] == "blob-ancestry", nf)
        finally:
            os.environ.clear()
            os.environ.update(old_env)

    print("[layer_ledger] self-test PASS", file=sys.stderr)
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    for name in ("common-layer", "sync-point", "divergence", "validate", "append", "merge"):
        sp = sub.add_parser(name)
        if name != "validate":
            sp.add_argument("--repo", default=".")
        if name in ("sync-point", "divergence"):
            sp.add_argument("--branch", required=True)
            sp.add_argument("--counterpart", default=None,
                            help="상대 브랜치(착지 확인 · 동기 지점이 없을 때 blob 조상 판정)")
            sp.add_argument("--ledger-ref", action="append", default=[],
                            help="원장을 **읽기만** 할 ref(반복 가능). 워크트리 원장과 entry_id 합집합")
        if name == "divergence":
            sp.add_argument("--since", default=None)
        if name in ("append", "validate"):
            sp.add_argument("--entry-file", required=True)
        if name == "merge":
            sp.add_argument("--from-ref", required=True)
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.cmd:
        ap.print_usage(sys.stderr)
        return EXIT_USAGE

    try:
        if args.cmd == "validate":
            problems = validate_entry(json.loads(Path(args.entry_file).read_text(encoding="utf-8")))
            res = {"status": "rejected" if problems else "valid", "problems": problems}
        else:
            repo = Path(args.repo).resolve()
            if args.cmd == "common-layer":
                res = common_layer(repo)
            elif args.cmd == "sync-point":
                res = sync_point(repo, args.branch, counterpart=args.counterpart, ledger_refs=args.ledger_ref)
            elif args.cmd == "divergence":
                res = divergence(repo, args.branch, args.since, counterpart=args.counterpart,
                                 ledger_refs=args.ledger_ref)
            elif args.cmd == "append":
                res = append(repo, json.loads(Path(args.entry_file).read_text(encoding="utf-8")))
            else:
                res = merge(repo, args.from_ref)
    except (OSError, ValueError, RuntimeError) as exc:
        print(json.dumps({"status": "error", "detail": f"{type(exc).__name__}: {exc}"}, ensure_ascii=False))
        return EXIT_USAGE

    print(json.dumps(res, ensure_ascii=False, indent=2))
    if args.cmd == "divergence" and res.get("diverged"):
        return EXIT_RED
    if args.cmd in ("append", "validate") and res.get("status") == "rejected":
        return EXIT_RED
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
