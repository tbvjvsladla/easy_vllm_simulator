#!/usr/bin/env python3
"""constitution_lint.py -- 헌법 품질 린트 (**정보이지 판정이 아니다**)

사용법:
  python3 .claude/policies/runtime/constitution_lint.py [--repo .] [--format json|text] [--self-test]

무엇이고 무엇이 아닌가
----------------------
이 린트는 **WARN 만** 낸다. 종료코드는 입력을 읽을 수 없을 때(2)를 빼면 항상 0 이다 -- 정량 임계·
삭제 목표·ablation 강제를 두지 않는 것이 사용자 결정이기 때문이다(plan_26091210 §3.8 · 인터뷰 Q4·Q6).
숫자를 **보여 주되 그 숫자로 차단하지 않는다**. 차단은 tripwire 8(4자일치·어휘)의 몫이다.

세 가지를 본다
--------------
1. **파일 간 중복 문장** -- 같은 문장이 두 헌법 파일에 있으면 red flag 다. 외부 참조에서 채택한 것은
   품질 기준 하나이고(템플릿 구조·일반 행동수칙 본문은 채택하지 않았다), 헌법 자신의 불변식
   "같은 규칙을 두 층에 적지 않는다"와 같은 말이다.
2. **상시 로드 바이트** -- 매 세션 무조건 읽히는 것의 크기. '매번 읽힐 가치' 기준만 채택했고
   80% 삭제·주기 삭제는 채택하지 않았다. 임계 없음.
3. **유령 정책 ID** -- 산문의 `policy:<ID>` 가 `registry.yaml` 에 실재하는가. 2026-09-12 까지
   `RUNNER_LADDER_ROTATION` 이 세 자리에서 등재된 정책처럼 읽혔고 아무도 대조하지 않았다
   (`citation_violations()` 는 호출자가 0 이다). 절 접미사(`.C1`)는 잘라서 읽으므로 정당한
   절-수준 인용을 위양성으로 잡지 않는다.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
from pathlib import Path

EXIT_OK = 0
EXIT_UNREADABLE = 2

#: 매 세션 무조건 로드되는 것. `.claude/rules/*.md` 는 `paths:` frontmatter 가 없으면 상시 로드다.
ALWAYS_LOADED = ("CLAUDE.md", ".claude/rules/workflow.md", ".claude/rules/docs.md")

_POLICY_REF = re.compile(r"policy:([A-Z][A-Z0-9_]{3,})")
#: 산문 잡음. 표 구분선·헤딩 장식·순수 기호 줄은 중복이라도 의미가 없다.
_NOISE = re.compile(r"^[\s|:#>*+\-=`_.]*$")
MIN_SENTENCE_CHARS = 40

#: 중복 검사에서 빼는 **계약 줄**. 특화 파일마다 반드시 있어야 하는 자기선언 헤더는 정의상 모든
#: 특화 파일에 같은 모양으로 나타난다 -- 그것을 "같은 규칙이 두 층에 적혔다"로 읽으면 이 린트가
#: 자기가 요구한 계약을 결함이라고 부르게 된다. 규칙의 중복이 아니라 **형식의 반복**이다.
_CONTRACT_LINES = (re.compile(r"^\*\*topology:"),)


def _layer_pathspec(root: Path) -> str:
    """특화 파일 경로 규약을 **소유자에게서** 읽는다(여기서 리터럴을 두 번째로 적지 않는다)."""
    path = root / ".claude/skills/terraforming_node/scripts/topology_parity.py"
    if not path.is_file():
        return "*.topology.md"
    spec = importlib.util.spec_from_file_location("_constitution_lint_topology_parity", path)
    if spec is None or spec.loader is None:
        return "*.topology.md"
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
    except Exception:  # noqa: BLE001
        return "*.topology.md"
    return getattr(module, "LAYER_PATHSPEC", "*.topology.md")


def _git_ls(root: Path, *pathspecs: str) -> list[str]:
    try:
        proc = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--", *pathspecs],
                              capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return []
    if proc.returncode != 0:
        return []
    return [r for r in proc.stdout.split("\0") if r]


def constitution_files(root: Path) -> list[str]:
    """중복 검사 대상 = 헌법 본문 + rules + 특화층."""
    return sorted(set(_git_ls(root, "CLAUDE.md", ".claude/rules/*.md", _layer_pathspec(root))))


def prose_files(root: Path) -> list[str]:
    """`policy:` 인용이 살 수 있는 산문 전수 -- 스킬 문서까지 본다.

    유령 ID 세 자리 중 하나가 스킬 문서에 있었다. 헌법만 훑으면 그 자리를 영원히 못 본다.
    """
    return sorted(set(constitution_files(root) + _git_ls(root, ".claude/skills/*/SKILL.md")))


def _normalize(line: str) -> str:
    s = re.sub(r"\s+", " ", line.strip())
    return re.sub(r"[`*_]", "", s)


def duplicate_lines(root: Path) -> list[dict]:
    """두 개 이상의 헌법 파일에 동일하게 나타나는 문장."""
    index: dict[str, set[str]] = {}
    for rel in constitution_files(root):
        try:
            text = (root / rel).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for raw in text.splitlines():
            norm = _normalize(raw)
            if len(norm) < MIN_SENTENCE_CHARS or _NOISE.match(raw):
                continue
            if any(rx.match(raw.strip()) for rx in _CONTRACT_LINES):
                continue
            index.setdefault(norm, set()).add(rel)
    out = [{"sentence": norm[:160], "files": sorted(files)}
           for norm, files in index.items() if len(files) > 1]
    return sorted(out, key=lambda d: (-len(d["files"]), d["sentence"]))


def always_loaded_bytes(root: Path) -> dict:
    sizes: dict[str, int | None] = {}
    for rel in ALWAYS_LOADED:
        p = root / rel
        sizes[rel] = p.stat().st_size if p.is_file() else None
    for rel in _git_ls(root, ".claude/rules/*.topology.md"):
        sizes[rel] = (root / rel).stat().st_size
    known = [v for v in sizes.values() if isinstance(v, int)]
    return {"files": sizes, "total_bytes": sum(known),
            "note": "임계 없음 -- 숫자는 보여 주되 이 린트는 차단하지 않는다"}


def ghost_policy_ids(root: Path) -> list[dict]:
    reg = root / ".claude/policies/registry.yaml"
    try:
        declared = {p.get("policy_id")
                    for p in json.loads(reg.read_text(encoding="utf-8"))["policies"]}
    except (OSError, ValueError, KeyError, TypeError) as exc:
        return [{"error": f"registry 를 읽지 못했다({exc}): {reg}"}]
    ghosts: dict[str, list[str]] = {}
    for rel in prose_files(root):
        try:
            text = (root / rel).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for m in _POLICY_REF.finditer(text):
            if m.group(1) not in declared:
                ghosts.setdefault(m.group(1), []).append(rel)
    return [{"policy_id": k, "referenced_in": sorted(set(v)), "count": len(v)}
            for k, v in sorted(ghosts.items())]


def lint(root: Path) -> dict:
    return {
        "schema_version": 1,
        "verdict": "INFO",
        "duplicate_sentences": duplicate_lines(root),
        "always_loaded": always_loaded_bytes(root),
        "ghost_policy_ids": ghost_policy_ids(root),
    }


def _render(res: dict) -> str:
    out = ["[constitution_lint] 정보 -- 차단하지 않는다"]
    al = res["always_loaded"]
    out.append(f"  상시 로드 합계 {al['total_bytes']:,} bytes")
    for rel, size in al["files"].items():
        out.append(f"    {size:>9,} {rel}" if isinstance(size, int) else f"    {'부재':>9} {rel}")
    dups = res["duplicate_sentences"]
    out.append(f"  파일 간 중복 문장 {len(dups)}건"
               + (" -- red flag: 같은 규칙이 두 층에 있다" if dups else ""))
    for d in dups[:20]:
        out.append(f"    {' · '.join(d['files'])}")
        out.append(f"      {d['sentence']}")
    if len(dups) > 20:
        out.append(f"    ... 외 {len(dups) - 20}건(전수는 --format json)")
    ghosts = res["ghost_policy_ids"]
    out.append(f"  유령 정책 ID {len(ghosts)}건")
    for g in ghosts:
        out.append(f"    {g.get('policy_id', g.get('error'))} -- {g.get('referenced_in')}")
    return "\n".join(out)


def _self_test() -> int:
    """격리 픽스처 양성·음성 대조. `assert` 를 쓰지 않는다(`-O` 에서 사라지는 검사는 검사가 아니다)."""
    import tempfile

    def ck(name: str, cond: bool, got: object = "") -> None:
        if not cond:
            raise RuntimeError(f"[constitution_lint] self-test FAIL: {name} (got={got!r})")

    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        subprocess.run(["git", "init", "-q", "-b", "single-node", str(root)], check=True)
        (root / ".claude/rules").mkdir(parents=True)
        (root / ".claude/policies").mkdir(parents=True)
        (root / ".claude/policies/registry.yaml").write_text(
            json.dumps({"schema_version": 2, "policies": [{"policy_id": "REAL_ONE"}]}),
            encoding="utf-8")
        shared = "이 문장은 두 파일에 똑같이 적혀 있어 중복으로 잡혀야 하는 충분히 긴 문장이다."
        (root / "CLAUDE.md").write_text(
            f"# c\n{shared}\npolicy:REAL_ONE 과 policy:GHOST_ONE 과 policy:REAL_ONE.C2 를 쓴다.\n",
            encoding="utf-8")
        (root / ".claude/rules/workflow.md").write_text(f"# w\n{shared}\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(root), "add", "-A"], check=True,
                       capture_output=True)

        res = lint(root)
        dup_files = [set(d["files"]) for d in res["duplicate_sentences"]]
        ck("양성: 파일 간 중복 문장 검출",
           {"CLAUDE.md", ".claude/rules/workflow.md"} in dup_files, dup_files)
        ids = {g["policy_id"] for g in res["ghost_policy_ids"]}
        ck("유령 ID 는 GHOST_ONE 하나 -- 절 접미사 인용은 위양성이 아니다", ids == {"GHOST_ONE"}, ids)
        ck("상시 로드 바이트 > 0", res["always_loaded"]["total_bytes"] > 0)

        (root / ".claude/rules/workflow.md").write_text(
            "# w\n다른 내용이며 충분히 길지만 중복은 아닌 이 문장 하나만 남긴다.\n", encoding="utf-8")
        ck("음성: 중복이 없으면 0건", not lint(root)["duplicate_sentences"])

    print("[constitution_lint] self-test PASS", file=sys.stderr)
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=".")
    ap.add_argument("--format", choices=("json", "text"), default="text")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()
    root = Path(args.repo).resolve()
    if not (root / "CLAUDE.md").is_file():
        print(f"[constitution_lint] CLAUDE.md 가 없다: {root}", file=sys.stderr)
        return EXIT_UNREADABLE
    res = lint(root)
    print(json.dumps(res, ensure_ascii=False, indent=2) if args.format == "json" else _render(res))
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
