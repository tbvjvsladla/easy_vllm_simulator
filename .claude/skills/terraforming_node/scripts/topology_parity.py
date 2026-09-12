#!/usr/bin/env python3
"""topology_parity.py -- 브랜치 ↔ 특화헌법 ↔ manifest ↔ 활성캠페인 **4자일치** 술어 (단일 소유)

사용법:
  python3 .claude/skills/terraforming_node/scripts/topology_parity.py evaluate \
      --repo . [--expect single|multi] [--format json|value]

왜 이 파일이 있나
-----------------
헌법은 "토폴로지는 manifest 가 권위이고 브랜치로 추론하지 않는다"고 선언해 왔지만 **그 선언을
집행하는 실행자가 0** 이었다(`docs/report/audit_26091123` §3: 반대 방향으로 파생하는 코드는 13곳).
그래서 2026-09-11 에 멀티 캠페인 14셀이 `single-node` 체크아웃에서 돌았고, 울린 트립와이어는 사람뿐이었다.

이 술어는 브랜치를 **권위가 아니라 필터**로 되돌린다 -- 브랜치는 "어느 헌법 특화층과 어느
`output/<t>/` 를 읽는가"를 고를 뿐이고, 그 선택이 나머지 셋과 어긋나면 fail-closed 로 막는다.
정책 `BRANCH_CONSTITUTION_LAYERING` C5.

**자동 교정은 하지 않는다.** 어긋남의 해소가 체크아웃 전환인지 선언 수정인지는 사람이 정한다 --
자동으로 한쪽에 맞추면 그 순간 권위가 다시 뒤집힌다.

네 다리
-------
| 다리 | 입력 | 성격 |
|---|---|---|
| branch   | `git symbolic-ref --short HEAD` | 필터(통로 선택기) |
| layer    | `git ls-files -- '*.topology.md'` 각 파일 머리의 `**topology: …**` 줄 | 특화헌법 자기선언 |
| manifest | `output/<t>/manifest.yaml` 의 `topology` | **토폴로지 사실의 권위** |
| campaign | 활성 `campaigns/<ACTIVE>/campaign.yaml` 의 `control_variables.topology` + `nodes[].topology` | 실행 통제변인 |

**부재와 불일치를 가른다**(2026-09-12 · plan_26091210 A1). 이 술어가 보는 입력 중 `manifest` 와
`campaign` 은 **비추적**이라 fresh clone 과 `git worktree` 체크아웃에는 아예 없다. 부재를 위반으로
세면 미테라포밍 클론이 커밋조차 못 하고 종료 시퀀스가 자기 커밋을 막는다. 절단선은 이렇다:

> **존재는 `policy:TERRAFORM_FLAG_GATE` 의 몫이고, 일치는 이 정책의 몫이다.**
> 추적 입력(특화 파일)의 부재는 RED. 비추적 입력의 부재는 `absent` 로 **표시**하되 위반이 아니다.
> 있는데 어긋나면 RED.

표시는 침묵이 아니다 -- 출력이 몇 개의 다리로 판정했는지(`legs_checked`)를 항상 밝히므로,
3자로 판정한 것이 4자로 판정한 것처럼 읽히지 않는다.

종료코드: 0 = PASS · 2 = usage · 5 = 계약 위반(RED). `node_role_contract.py` 규약과 같다.
stdout 은 JSON(또는 `--format value` 의 한 줄) 전용이고 진단은 stderr 다 -- 셸 소비자가 stdout 을 파싱한다.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_CONTRACT_VIOLATION = 5

#: 특화헌법 파일의 **경로 규약**. 정책 `BRANCH_CONSTITUTION_LAYERING` C1 이 이 문자열의 권위다.
#:
#: 형태가 `*.topology.md` 이고 `**/*.topology.md` 가 아닌 이유(2026-09-12 실증): git pathspec 의
#: 선행 `**/` 는 디렉터리 **0개를 매치하지 않는다**(`:(exclude)**/*.md` 는 루트의 .md 3건을 남긴다).
#: 반면 Python `pathlib` 의 `**` 는 0개를 매치한다. 두 형태를 섞으면 *술어가 보는 집합* 과
#: *sync 가 제외하는 집합* 이 갈린다 -- 같은 규칙이 두 자리에 다른 의미로 적히는 전형이다.
#: 그래서 열거도 제외도 **git pathspec 한 문법**만 쓴다(아래 `layer_files` 는 `pathlib.glob` 을
#: 쓰지 않는다). 셸 쪽 리터럴과의 교차검증은 `runtime_selftest` 가 맡는다.
LAYER_SUFFIX = ".topology.md"
LAYER_PATHSPEC = "*" + LAYER_SUFFIX
LAYER_EXCLUDE_PATHSPEC = ":(exclude)" + LAYER_PATHSPEC

#: 자기선언 헤더의 **정확한 형식**. 파일 머리 `HEADER_WINDOW` 줄 안에 정확히 한 번 나와야 한다.
#:
#: frontmatter 가 아니라 **본문**에 두는 이유(2026-09-12 확정): rules 파일의 인식되지 않는
#: frontmatter 키가 어떻게 처분되는지는 **문서화된 동작이 아니고**, 블록 HTML 주석은 컨텍스트
#: 주입 전에 제거된다. 둘 중 어느 쪽이든 *이 술어가 읽는 바이트* 와 *에이전트가 읽는 컨텍스트* 가
#: 갈릴 수 있고, 갈라진 쪽은 조용히 늦는다. 본문 가시 선언은 둘이 같은 자리를 읽게 해 그 갈라짐을
#: 구조적으로 없앤다. 관대한 파싱 대신 **닫힌 형식**을 요구하는 것도 같은 이유다.
_HEADER_DECL = re.compile(r"^\*\*topology:[ \t]*(single|multi)\*\*")
HEADER_WINDOW = 10

BRANCH_TO_TOPOLOGY = {"single-node": "single", "multi-node": "multi"}
TOPOLOGIES = ("single", "multi")


def _git(repo: Path, *args: str) -> str | None:
    try:
        proc = subprocess.run(["git", "-C", str(repo), *args],
                              capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    return proc.stdout.strip() if proc.returncode == 0 else None


def read_layer_header(path: Path) -> tuple[str | None, str]:
    """특화 파일 머리의 자기선언을 읽는다. 반환 `(topology|None, 사유)`.

    형식: 파일 머리 `HEADER_WINDOW` 줄 안에 `**topology: single|multi**` 로 시작하는 줄이
    **정확히 하나**. 0개면 선언 없음, 2개 이상이면 어느 것이 선언인지 모른다 -- 둘 다 RED 다.
    yaml 파서를 부르지 않는다: 읽는 것은 닫힌 한 줄이고, 이 함수는 pre-commit 1초 예산 안에서 돈다.
    """
    try:
        with path.open(encoding="utf-8") as fh:
            hits: list[str] = []
            for _ in range(HEADER_WINDOW):
                line = fh.readline()
                if line == "":
                    break
                m = _HEADER_DECL.match(line)
                if m:
                    hits.append(m.group(1))
    except (OSError, UnicodeDecodeError) as exc:
        return None, f"읽기 실패: {exc}"
    if not hits:
        return None, (f"머리 {HEADER_WINDOW}줄 안에 자기선언 헤더가 없다 "
                      "(`**topology: single|multi**` 로 시작하는 줄이 있어야 한다)")
    if len(hits) > 1:
        return None, f"자기선언 헤더가 {len(hits)}개다 -- 어느 것이 선언인지 모른다: {hits}"
    return hits[0], "ok"


def layer_files(repo: Path) -> list[Path]:
    """특화헌법 파일 전수. **추적물만** 보며 열거는 git 이 한다(`LAYER_PATHSPEC` 주석 참조).

    `-z` NUL 열거인 이유: `--name-only` 의 기본 quotePath 가 비-ASCII 경로를 이스케이프해
    거짓 drift 를 만든 선례가 있다.
    """
    try:
        proc = subprocess.run(["git", "-C", str(repo), "ls-files", "-z", "--", LAYER_PATHSPEC],
                              capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return []
    if proc.returncode != 0:
        return []
    return sorted(repo / rel for rel in proc.stdout.split("\0") if rel)


def _load_manifest(path: Path) -> dict:
    import yaml  # 지연 import -- manifest 다리에 도달하기 전에는 필요 없다
    with path.open(encoding="utf-8") as fh:
        return yaml.safe_load(fh) or {}


def _campaign_module(repo: Path):
    """`campaign_init` 을 파일 경로로 적재한다(ACTIVE 해소·선언 읽기의 **단일 권위**).

    포인터 규약을 여기서 다시 구현하지 않는 이유: 같은 규칙이 두 자리에 적히면 갈라지고, 갈라진
    쪽이 조용히 늦는다. 적재 비용은 실측 3ms 다.
    """
    path = repo / ".claude/skills/terraforming_node/scripts/campaign_init.py"
    if not path.is_file():
        return None
    spec = importlib.util.spec_from_file_location("_topology_parity_campaign_init", path)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
    except Exception:  # noqa: BLE001 -- 적재 실패는 다리 결측이지 이 술어의 크래시가 아니다
        return None
    return module


def evaluate(repo: str | os.PathLike[str] = ".", *, expect: str | None = None) -> dict:
    """4자일치를 판정한다. 부작용 없음 -- 읽기만 한다."""
    root = Path(repo).resolve()
    legs: dict[str, dict] = {}
    reasons: list[str] = []

    # ── 다리 1: 브랜치 = 통로 필터 ──────────────────────────────────────────
    #
    # `symbolic-ref` 를 먼저 묻는 이유(2026-09-12 픽스처가 드러냈다): `rev-parse --abbrev-ref HEAD`
    # 는 **커밋이 하나도 없는 저장소에서 실패한다**. 그런데 pre-commit 훅은 바로 그 시점 -- 첫
    # 커밋이 아직 없는 순간 -- 에도 돈다. 그 자리에서 브랜치를 못 읽으면 이 술어는 "브랜치 불명"
    # 이라고 거짓말을 하게 된다. `symbolic-ref` 는 미탄생 브랜치에서도 이름을 돌려주고, detached
    # HEAD 에서는 실패한다 -- 후자는 통로가 정말 없는 상태이므로 BRANCH_UNKNOWN 이 옳다.
    branch = (_git(root, "symbolic-ref", "--quiet", "--short", "HEAD")
              or _git(root, "rev-parse", "--abbrev-ref", "HEAD"))
    topology = BRANCH_TO_TOPOLOGY.get(branch or "")
    legs["branch"] = {"value": topology, "raw": branch,
                      "source": "git symbolic-ref --short HEAD (fallback: rev-parse --abbrev-ref)",
                      "status": "ok" if topology else "red"}
    if topology is None:
        reasons.append(
            f"BRANCH_UNKNOWN: 브랜치 {branch!r} 에서 토폴로지 통로를 고를 수 없다 -- "
            "`single-node`/`multi-node` 중 하나를 체크아웃하거나 호출부가 --expect 를 명시해야 한다 "
            "(unknown 을 single 로 삼던 침묵 기본값은 폐지됐다)")

    # ── 다리 2: 특화헌법 자기선언 (추적물 -- 부재는 RED) ────────────────────
    found = layer_files(root)
    headers = []
    for path in found:
        value, why = read_layer_header(path)
        headers.append({"path": str(path.relative_to(root)), "topology": value, "detail": why})
    declared = {h["topology"] for h in headers if h["topology"]}
    legs["layer"] = {"value": (next(iter(declared)) if len(declared) == 1 else None),
                     "files": headers, "count": len(found),
                     "source": f"git ls-files -- {LAYER_PATHSPEC} (본문 자기선언)",
                     "status": "ok"}
    if not found:
        legs["layer"]["status"] = "red"
        reasons.append(
            f"LAYER_ABSENT: 특화헌법 파일이 0개다 -- 추적 트리에 `{LAYER_PATHSPEC}` 가 하나도 없으면 "
            "이 체크아웃은 어느 토폴로지의 헌법을 읽고 있는지 스스로 말하지 못한다")
    else:
        for h in headers:
            if not h["topology"]:
                legs["layer"]["status"] = "red"
                reasons.append(f"LAYER_HEADER_MALFORMED: {h['path']} -- {h['detail']}")
            elif topology and h["topology"] != topology:
                legs["layer"]["status"] = "red"
                reasons.append(
                    f"LAYER_HEADER_MISMATCH: {h['path']} 는 topology={h['topology']} 를 선언하는데 "
                    f"체크아웃 브랜치는 {branch}({topology}) 다 -- 반대 브랜치의 특화 파일이 흘러들어왔다")
        if len(declared) > 1:
            legs["layer"]["status"] = "red"
            reasons.append(
                f"LAYER_HEADER_SPLIT: 한 체크아웃에 서로 다른 topology 선언이 섞여 있다: {sorted(declared)}")

    # ── 다리 3: manifest = 토폴로지 사실의 권위 (비추적 -- 부재는 absent) ────
    mtopo = topology or expect
    leg3: dict = {"source": "output/<t>/manifest.yaml", "status": "ok", "value": None,
                  "path": (f"output/{mtopo}/manifest.yaml" if mtopo else None)}
    if mtopo is None:
        leg3["status"] = "skipped"
        leg3["detail"] = "통로가 정해지지 않아 읽을 manifest 가 없다(다리 1 RED 에 종속)"
    else:
        mpath = root / "output" / mtopo / "manifest.yaml"
        if not mpath.is_file():
            leg3["status"] = "absent"
            leg3["detail"] = ("manifest 가 없다 -- 비추적 산출물이라 미테라포밍 클론·워크트리에는 "
                              "없는 것이 정상이다. **존재**는 policy:TERRAFORM_FLAG_GATE 가 요구하고, "
                              "이 술어는 **일치**만 본다")
        else:
            try:
                man = _load_manifest(mpath)
            except Exception as exc:  # noqa: BLE001
                man = None
                leg3["status"] = "red"
                reasons.append(f"MANIFEST_UNREADABLE: output/{mtopo}/manifest.yaml 파싱 실패: {exc}")
            if isinstance(man, dict):
                leg3["value"] = man.get("topology")
                terra = man.get("terraforming") or {}
                leg3["terraforming_complete"] = bool(terra.get("complete"))
                leg3["branch_verified"] = bool(terra.get("branch_verified"))
                if leg3["value"] != mtopo:
                    leg3["status"] = "red"
                    reasons.append(
                        f"MANIFEST_MISMATCH: output/{mtopo}/manifest.yaml#topology={leg3['value']!r} 인데 "
                        f"통로는 {mtopo!r} 다 -- manifest 가 권위이므로 통로 선택이 틀렸거나 manifest 가 오염됐다")
    legs["manifest"] = leg3

    # ── 다리 4: 활성 캠페인 통제변인 (비추적 -- 부재는 absent) ──────────────
    leg4: dict = {"source": "campaigns/<ACTIVE>/campaign.yaml", "status": "ok", "value": None}
    mod = _campaign_module(root)
    if mod is None:
        leg4["status"] = "absent"
        leg4["detail"] = ("campaign_init.py 를 적재하지 못했다 -- ACTIVE 해소 규약의 단일 권위가 "
                          "없으면 이 다리는 판정하지 않는다")
    else:
        camp_id = mod.active_campaign_id(str(root))
        leg4["campaign_id"] = camp_id
        if camp_id == mod.BOOTSTRAP:
            leg4["status"] = "not-applicable"
            leg4["detail"] = ("활성 캠페인이 없다(ACTIVE=_bootstrap) -- 캠페인 밖 평시 작업에서는 정상이다. "
                              "이번 판정은 이 다리를 빼고 이뤄졌다")
        else:
            decl = mod.read_declaration(root / "campaigns" / camp_id)
            cv = (decl.get("control_variables") or {}).get("topology")
            nodes = [[n.get("node_id"), n.get("topology")] for n in (decl.get("nodes") or [])]
            leg4["value"] = cv
            leg4["nodes"] = nodes
            target = topology or expect
            if cv not in TOPOLOGIES:
                leg4["status"] = "red"
                reasons.append(
                    f"CAMPAIGN_TOPOLOGY_UNDECLARED: {camp_id} 의 control_variables.topology={cv!r} -- "
                    "선언이 비면 실행자가 저장소 기본값으로 되돌아간다")
            elif target and cv != target:
                leg4["status"] = "red"
                reasons.append(
                    f"CAMPAIGN_MISMATCH: 활성 캠페인 {camp_id} 는 topology={cv!r} 로 선언됐는데 "
                    f"체크아웃은 {target!r} 통로다 -- 이 조합이 2026-09-11 사고의 형태다")
            bad = [n for n in nodes if n[1] and cv and n[1] != cv]
            if bad:
                leg4["status"] = "red"
                reasons.append(f"CAMPAIGN_NODE_MISMATCH: 노드 선언이 캠페인 통제변인과 어긋난다: {bad}")
    legs["campaign"] = leg4

    # ── 호출부 기대값 ───────────────────────────────────────────────────────
    if expect is not None:
        if expect not in TOPOLOGIES:
            reasons.append(f"EXPECT_INVALID: --expect {expect!r} 는 single|multi 가 아니다")
        elif topology is not None and expect != topology:
            reasons.append(
                f"EXPECT_MISMATCH: 호출부는 {expect!r} 를 기대하는데 체크아웃 통로는 {topology!r} 다")

    judged = sorted(k for k, v in legs.items()
                    if v.get("status") not in ("skipped", "absent", "not-applicable"))
    verdict = "PASS" if not reasons else "RED"
    return {
        "schema_version": 1,
        "predicate": "topology_parity",
        "policy": "BRANCH_CONSTITUTION_LAYERING.C5",
        "verdict": verdict,
        "topology": topology,
        "legs_checked": len(judged),
        "legs_judged": judged,
        "legs": legs,
        "reason_codes": [r.split(":", 1)[0] for r in reasons],
        "reasons": reasons,
        "remediation": (None if verdict == "PASS" else
                        "자동 교정하지 않는다 -- 체크아웃 전환인지 선언 수정인지는 사람이 정한다. "
                        "고친 뒤 이 술어를 다시 돌려라."),
    }


def _self_test() -> int:
    """헤더 파서의 양성·음성 대조. 라이브 트리를 요구하지 않는다.

    `assert` 를 쓰지 않는다 -- `-O` 에서 사라지는 검사는 검사가 아니고, 이 저장소의
    `runtime_selftest._test_no_production_asserts` 가 `.claude/**/*.py` 전수에서 그것을 막는다.
    """
    import tempfile

    def ck(name: str, cond: bool, got: object = "") -> None:
        if not cond:
            raise RuntimeError(f"[topology_parity] self-test FAIL: {name} (got={got!r})")

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        good = root / "good.topology.md"
        good.write_text("# t\n\n**topology: multi** · layer: topology\n\n> 본문\n", encoding="utf-8")
        ck("양성: 선언을 읽는다", read_layer_header(good) == ("multi", "ok"), read_layer_header(good))

        bad = root / "bad.topology.md"
        bad.write_text("# t\n\n선언이 없다\n", encoding="utf-8")
        ck("음성: 선언 부재", read_layer_header(bad)[0] is None, read_layer_header(bad))

        dup = root / "dup.topology.md"
        dup.write_text("**topology: multi**\n**topology: single**\n", encoding="utf-8")
        dup_res = read_layer_header(dup)
        ck("음성: 선언 2개는 모호하므로 거부", dup_res[0] is None and "2개" in dup_res[1], dup_res)

        late = root / "late.topology.md"
        late.write_text("\n" * HEADER_WINDOW + "**topology: single**\n", encoding="utf-8")
        ck("음성: 창 밖 선언은 읽지 않는다", read_layer_header(late)[0] is None, read_layer_header(late))

        loose = root / "loose.topology.md"
        loose.write_text("topology: multi\n", encoding="utf-8")
        ck("음성: 닫힌 형식만 받는다", read_layer_header(loose)[0] is None, read_layer_header(loose))

        empty = root / "empty.topology.md"
        empty.write_text("", encoding="utf-8")
        ck("음성: 빈 파일", read_layer_header(empty)[0] is None, read_layer_header(empty))

    # 비-git 디렉터리에서 열거는 빈 목록이지 크래시가 아니다.
    with tempfile.TemporaryDirectory() as td:
        ck("비-git 트리에서 열거는 빈 목록", layer_files(Path(td)) == [])

    print("[topology_parity] self-test PASS (6 헤더 케이스 + 열거 1)", file=sys.stderr)
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true", help="헤더 파서 자체검사")
    sub = ap.add_subparsers(dest="cmd")
    ev = sub.add_parser("evaluate", help="4자일치 판정")
    ev.add_argument("--repo", default=".")
    ev.add_argument("--expect", choices=list(TOPOLOGIES), default=None,
                    help="호출부가 기대하는 토폴로지(어긋나면 RED)")
    ev.add_argument("--format", choices=("json", "value"), default="json")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if args.cmd != "evaluate":
        ap.print_usage(sys.stderr)
        return EXIT_USAGE

    res = evaluate(args.repo, expect=args.expect)
    if args.format == "value":
        if res["verdict"] != "PASS" or not res["topology"]:
            for r in res["reasons"]:
                print(f"[topology_parity] {r}", file=sys.stderr)
            return EXIT_CONTRACT_VIOLATION
        print(res["topology"])
        return EXIT_OK
    print(json.dumps(res, ensure_ascii=False, indent=2, sort_keys=True))
    return EXIT_OK if res["verdict"] == "PASS" else EXIT_CONTRACT_VIOLATION


if __name__ == "__main__":
    sys.exit(main())
