#!/usr/bin/env python3
"""selftest_branch_sync.py -- 브랜치 동기화·서브 전파 기구의 **격리 픽스처 실행 검사** (⑧-pre D2 · 2026-09-14)

사용법:
  python3 .claude/skills/upstream-version-watch/scripts/selftest_branch_sync.py

왜 이 파일이 있나
-----------------
⑧(브랜치 동기화·서브 전파) 착수 직전 분석이 기구 결함 여섯을 찾았다. 공통점은 **실행해 보지 않으면 보이지 않는
배선**이라는 것이다 -- 단위 함수는 옳은데 셸이 그 함수를 옛 판본으로 부르거나(갈라짐 판정기), 가드가 있어야 할
자리에 호출이 없거나(서브 전파 토폴로지), 실패 원인이 한 종료코드로 뭉개졌다(카탈로그). 그래서 이 검사는 **배포되는
스크립트의 바이트 사본**을 임시 git 저장소에서 실제로 돌린다(호출자 없는 자체검사는 L1 산문이다 -- verify_distribution
이 매번 부른다):

  S6  sync_to_sub.sh 진입 가드 -- 멀티 체크아웃 + --branch single · 싱글 체크아웃 + 기본값 multi · both · 헤더 드리프트는
      **인가 이전·부작용 0** 으로 exit 12, 정상 조합은 가드를 지나 인가 게이트에 닿는다. 가드 호출을 걷어낸 변이 사본이
      적색이 되는지로 이 검사 자신을 친다. render_sub_env.py CLI 도 같은 자리(렌더 전)에서 exit 5.
  S1  closing_sequence execute(multi→single)의 RED ② 가 대상 워크트리에 원장이 없어도 **실제 판정**(clean/diverged)을 낸다.
      대상의 판정기가 옛 판본이면 LEDGER_JUDGE_STALE 로 멈추고, 종료 시퀀스의 선행 당김이 그것을 푼다(변이 음성대조).
  S3  분류표 빈칸이 docs/*/example.md 변경을 보여 주고, 동기화가 그것을 옮긴다.
  S4  빈칸 그대로의 분류표는 execute 가 **어떤 부작용보다 먼저** 거부한다 -- 빈칸 생성기의 자리표시자와 원장의 거부 규칙이
      같은 소유자에서 온다(빈칸 → validate → rejected).
  S7  원격 전용 hint 태그(로컬 오브젝트 부재)로 카탈로그 재파생이 실패하면 원인과 그 원격의 fetch 명령을 말하고,
      자동 fetch 없이 진행한다.

원격·서브·실 원장은 건드리지 않는다: 원격은 임시 bare 저장소, ssh 는 호출되면 흔적을 남기는 shim, 시각은 픽스처 상수다.
`assert` 는 쓰지 않는다(`-O` 에서 사라진다 -- runtime_selftest 가 전수 금지).
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REAL = Path(__file__).resolve().parents[4]
UVW = ".claude/skills/upstream-version-watch/scripts"
TN = ".claude/skills/terraforming_node/scripts"
SYNC_TO_SUB = f"{UVW}/sync_to_sub.sh"
SYNC_BRANCHES = f"{UVW}/sync_branches.sh"
LAYER_LEDGER = f"{UVW}/layer_ledger.py"
CLOSING = f"{UVW}/closing_sequence.sh"
PARITY = f"{TN}/topology_parity.py"
CAMPAIGN_INIT = f"{TN}/campaign_init.py"
RENDER = f"{TN}/render_sub_env.py"
GATE = ".claude/policies/runtime/completion_gate.py"
CATALOG = ".claude/skills/hint-publisher/scripts/hint_catalog.py"
SCHEMAS = ("work-manifest.schema.json", "completion-manifest.schema.json",
           "side-effect-authorization.schema.json")
LEDGER_REL = ".claude/policies/branch_layer_ledger.json"
TOPOLOGY_RULES = ".claude/rules/strategy.topology.md"
CERT_REL = "docs/benchmark/benchmark_26091400_fixture_gb10_v1.yaml"
GUARD_CALL = 'assert_delivery_topology || exit "$TOPOLOGY_GUARD_EXIT"\n'
PREPULL_JUDGE = "    .claude/skills/upstream-version-watch/scripts/layer_ledger.py\n"

FAILURES: list[str] = []


def ck(name: str, cond: bool, got: object = "") -> None:
    print(f"  {'ok  ' if cond else 'FAIL'} {name}", file=sys.stderr)
    if not cond:
        FAILURES.append(f"{name} (got={got!r})"[:1500])


class Sandbox:
    """임시 루트 · 격리 git 설정 · ssh shim. 실 저장소의 설정(훅·서명)은 스며들지 않는다."""

    def __init__(self, root: Path):
        self.root = root
        self.tmp = root / "tmp"
        self.tmp.mkdir()
        gcfg = root / "gitconfig"
        gcfg.write_text("[user]\n\tname = selftest\n\temail = t@t\n[commit]\n\tgpgsign = false\n"
                        "[tag]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n", encoding="utf-8")
        self.ssh_marker = root / "ssh_called"
        shim = root / "ssh_shim.sh"
        shim.write_text(f'#!/bin/sh\necho "$@" >> "{self.ssh_marker}"\nexit 255\n', encoding="utf-8")
        shim.chmod(0o755)
        self.env = {k: v for k, v in os.environ.items()
                    if k not in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                                 "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX",
                                 "GITHUB_TOKEN", "SRC", "SUB_HOST", "SUB_WORK_DIR")}
        self.env.update(GIT_CONFIG_GLOBAL=str(gcfg), GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0",
                        TMPDIR=str(self.tmp), SYNC_SSH_COMMAND=str(shim),
                        # 사본 트리에 __pycache__ 가 생기면 "부작용 0" 판정이 흐려진다 -- 바이트코드를 쓰지 않는다
                        PYTHONDONTWRITEBYTECODE="1")

    def git(self, cwd: Path, *args: str) -> str:
        proc = subprocess.run(["git", "-C", str(cwd), *args], capture_output=True, text=True,
                              env=self.env, timeout=120)
        if proc.returncode != 0:
            raise RuntimeError(f"fixture git {args} @ {cwd.name} failed: {proc.stderr.strip()[:400]}")
        return proc.stdout.strip()

    def run(self, argv: list[str], cwd: Path, extra: dict | None = None) -> subprocess.CompletedProcess:
        return subprocess.run(argv, capture_output=True, text=True, cwd=str(cwd),
                              env={**self.env, **(extra or {})}, timeout=600)


def _copy_real(dst_root: Path, rel: str) -> None:
    src = REAL / rel
    dst = dst_root / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


def _write(root: Path, rel: str, text: str) -> None:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


def _tree_snapshot(root: Path) -> str:
    """부작용 0 판정용: .git 밖 파일 전수(경로·바이트) 지문."""
    h = hashlib.sha256()
    for p in sorted(root.rglob("*")):
        if ".git" in p.relative_to(root).parts or not p.is_file():
            continue
        h.update(str(p.relative_to(root)).encode())
        h.update(p.read_bytes())
    return h.hexdigest()


# ════════════════════════════════════════════════════════════════════════════════════════════════
# S6 -- sync_to_sub.sh 진입 가드
# ════════════════════════════════════════════════════════════════════════════════════════════════

def _checkout(sb: Sandbox, name: str, branch: str, header: str, topo: str,
              manifest_topo: str | None = None, campaign_topo: str | None = None) -> Path:
    """`campaign_topo` 를 주면 활성 캠페인까지 선 체크아웃이다(4자일치 네 다리 전부 · 실물 운영 체크아웃의 형태)."""
    repo = sb.root / name
    repo.mkdir()
    sb.git(repo, "init", "-q", "-b", branch)
    for rel in (SYNC_TO_SUB, PARITY, GATE, CAMPAIGN_INIT, *(f".claude/schemas/{s}" for s in SCHEMAS)):
        _copy_real(repo, rel)
    _write(repo, TOPOLOGY_RULES, f"# 특화헌법 픽스처\n\n**topology: {header}** · layer: topology\n")
    # single-node 체크아웃도 옛 output/multi 빌드킷을 추적한다(다운그레이드 배달의 형태) -- 가드는 그 앞에서 멈춰야 한다.
    _write(repo, "output/multi/Dockerfile", "# fixture: 낡은 버전 핀을 든 빌드킷\n")
    sb.git(repo, "add", "-A")
    sb.git(repo, "commit", "-qm", "fixture checkout")
    # manifest 는 비추적 산출물이다(4자일치의 manifest 다리) -- 실물처럼 파일시스템에만 둔다.
    _write(repo, f"output/{topo}/manifest.yaml", f"topology: {manifest_topo or topo}\n")
    if campaign_topo:
        # 활성 캠페인도 비추적 인스턴스다 -- ACTIVE 포인터 규약은 campaign_init 이 소유한다(여기서는 그 모양만 놓는다).
        _write(repo, "campaigns/ACTIVE", "camp-fixture\n")
        _write(repo, "campaigns/camp-fixture/campaign.yaml",
               json.dumps({"control_variables": {"topology": campaign_topo}, "nodes": []}) + "\n")
    return repo


def _sync_to_sub(sb: Sandbox, repo: Path, *args: str, script: Path | None = None) -> subprocess.CompletedProcess:
    return sb.run(["bash", str(script or (repo / SYNC_TO_SUB)), "--mode", "experimental", *args], cwd=sb.root)


def test_sync_to_sub_guard() -> None:
    print("[S6] sync_to_sub.sh 배달 통로 ↔ 체크아웃 특화층 가드", file=sys.stderr)
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        multi = _checkout(sb, "multi", "multi-node", "multi", "multi")
        single = _checkout(sb, "single", "single-node", "single", "single")

        def no_side_effect(label: str, repo: Path, before: str) -> None:
            leaked = sorted(p.name for p in sb.tmp.iterdir())
            ck(f"{label}: 부작용 0 (트리 불변 · 트랜잭션 소스 0 · ssh 0 · 스테이징 0)",
               _tree_snapshot(repo) == before and not leaked and not sb.ssh_marker.exists()
               and not list(repo.glob("output/*/sub_provision")), (leaked, sb.ssh_marker.exists()))

        before = _tree_snapshot(multi)
        r = _sync_to_sub(sb, multi, "--branch", "single")
        ck("★S6 음성①: 멀티 체크아웃 + --branch single → exit 12 · TOPOLOGY_DELIVERY_MISMATCH · EXPECT_MISMATCH",
           r.returncode == 12 and "TOPOLOGY_DELIVERY_MISMATCH" in r.stderr and "EXPECT_MISMATCH" in r.stderr,
           (r.returncode, r.stderr[-600:]))
        ck("★S6 음성①: 인가 게이트에 닿기 전이다(게이트 JSON 없음)", "authorization_state" not in r.stdout, r.stdout[-300:])
        no_side_effect("S6 음성①", multi, before)

        before = _tree_snapshot(single)
        r = _sync_to_sub(sb, single)
        ck("★S6 음성②: 싱글 체크아웃 + 기본값(--branch multi) → exit 12 (옛 빌드킷 다운그레이드 배달 차단)",
           r.returncode == 12 and "TOPOLOGY_DELIVERY_MISMATCH" in r.stderr, (r.returncode, r.stderr[-600:]))
        # 가장 흔한 형태는 --branch 를 **잊은** 것이다 -- 안내가 "multi 체크아웃으로 가라" 만 말하면 사람을 반대 통로로 보낸다
        ck("★S6 음성② 안내: 이 체크아웃의 통로(--branch single)와 반대 체크아웃 두 선택지를 함께 말한다",
           "같은 명령에 --branch single" in r.stderr and "'multi' 토폴로지 브랜치를 체크아웃한 저장소" in r.stderr,
           r.stderr[-600:])
        no_side_effect("S6 음성②", single, before)

        # 체크아웃 **자체**가 4자일치 RED 인 경우(--branch 는 맞다) -- "그 브랜치를 체크아웃한 저장소에서 다시" 는 틀린 안내다
        man_bad = _checkout(sb, "man-bad", "multi-node", "multi", "multi", manifest_topo="single")
        before = _tree_snapshot(man_bad)
        r = _sync_to_sub(sb, man_bad, "--branch", "multi")
        ck("★S6 음성④: --branch 는 맞고 manifest 가 어긋남 → exit 12 · MANIFEST_MISMATCH · 체크아웃 자체 RED 로 안내(--branch 교체 제안 ✗)",
           r.returncode == 12 and "MANIFEST_MISMATCH" in r.stderr and "자체가 4자일치를 통과하지 못한다" in r.stderr
           and "같은 명령에 --branch" not in r.stderr, (r.returncode, r.stderr[-700:]))
        no_side_effect("S6 음성④", man_bad, before)
        # ⑧ 다음 싱글 서브 회차가 실제로 만날 형태: single-node 체크아웃 + 멀티 캠페인이 ACTIVE + --branch single
        camp_bad = _checkout(sb, "camp-bad", "single-node", "single", "single", campaign_topo="multi")
        before = _tree_snapshot(camp_bad)
        r = _sync_to_sub(sb, camp_bad, "--branch", "single")
        ck("★S6 음성⑤: 싱글 체크아웃 + 활성 멀티 캠페인 + --branch single → exit 12 · CAMPAIGN_MISMATCH 를 드러낸다",
           r.returncode == 12 and "CAMPAIGN_MISMATCH" in r.stderr and "활성 캠페인" in r.stderr
           and "같은 명령에 --branch" not in r.stderr, (r.returncode, r.stderr[-700:]))
        no_side_effect("S6 음성⑤", camp_bad, before)
        camp_ok = _checkout(sb, "camp-ok", "multi-node", "multi", "multi", campaign_topo="multi")
        r = _sync_to_sub(sb, camp_ok, "--branch", "multi")
        ck("★S6 양성(네 다리): 활성 캠페인까지 일치 → 가드 통과 · 판정 다리 수를 밝힌다(4/4 · C5 공개)",
           r.returncode not in (0, 12) and "배달 통로 대조 PASS" in r.stderr and "(4/4)" in r.stderr
           and "campaign" in r.stderr, (r.returncode, r.stderr[-500:]))

        r = _sync_to_sub(sb, multi, "--branch", "both")
        ck("★S6 both 거부 비회귀: exit 12 · TOPOLOGY_DELIVERY_BOTH(한 체크아웃은 특화층 하나)",
           r.returncode == 12 and "TOPOLOGY_DELIVERY_BOTH" in r.stderr, (r.returncode, r.stderr[-400:]))
        r = _sync_to_sub(sb, multi, "--branch", "bogus")
        ck("--branch 값역 밖은 종전대로 exit 6(파싱 단계)", r.returncode == 6, r.returncode)

        for repo, branch in ((multi, "multi"), (single, "single")):
            before = _tree_snapshot(repo)
            r = _sync_to_sub(sb, repo, "--branch", branch)
            try:
                gate = json.loads(r.stdout)
            except ValueError:
                gate = None
            ck(f"★S6 양성: {repo.name} 체크아웃 + --branch {branch} → 가드 통과 · 다음 문(인가 게이트)이 manifest 부재로 거부",
               r.returncode not in (0, 12) and "STOP(TOPOLOGY" not in r.stderr
               and isinstance(gate, dict) and gate.get("allowed") is False,
               (r.returncode, r.stdout[-300:], r.stderr[-300:]))
            ck(f"S6 양성({branch}): 통과 줄이 판정 다리를 밝힌다(캠페인 없는 체크아웃 = 3/4)",
               "배달 통로 대조 PASS" in r.stderr and "(3/4)" in r.stderr, r.stderr[-300:])
            no_side_effect(f"S6 양성({branch})", repo, before)

        # 헤더 드리프트: 브랜치는 multi-node 인데 특화헌법 자기선언이 single -- 4자일치 헤더 다리가 막는다
        drift = _checkout(sb, "drift", "multi-node", "single", "multi")
        r = _sync_to_sub(sb, drift, "--branch", "multi")
        ck("★S6 음성③: 헤더 드리프트(브랜치 multi-node · 자기선언 single) → exit 12 · LAYER_HEADER_MISMATCH",
           r.returncode == 12 and "LAYER_HEADER_MISMATCH" in r.stderr, (r.returncode, r.stderr[-400:]))

        # 변이 음성대조: 가드 호출을 걷어낸 사본은 음성①을 막지 못한다 -- 이 검사가 그 회귀를 잡는다
        src = (multi / SYNC_TO_SUB).read_text(encoding="utf-8")
        ck("변이 앵커(가드 호출 줄) 정확히 1건", src.count(GUARD_CALL) == 1, src.count(GUARD_CALL))
        mutated = multi / UVW / "sync_to_sub_mutated.sh"
        mutated.write_text(src.replace(GUARD_CALL, ""), encoding="utf-8")
        r = _sync_to_sub(sb, multi, "--branch", "single", script=mutated)
        ck("★S6 변이 음성대조: 가드 호출이 없으면 멀티 체크아웃 + single 이 인가 단계까지 간다(결함 재현)",
           r.returncode != 12 and "TOPOLOGY_DELIVERY" not in r.stderr, (r.returncode, r.stderr[-300:]))
        mutated.unlink()


def test_render_guard() -> None:
    print("[S6] render_sub_env.py 렌더 전 특화헌법 대조", file=sys.stderr)
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        repo = sb.root / "render"
        (repo / TN).mkdir(parents=True)
        for py in sorted((REAL / TN).glob("*.py")):
            shutil.copy2(py, repo / TN / py.name)
        _write(repo, TOPOLOGY_RULES, "# 특화헌법 픽스처\n\n**topology: multi** · layer: topology\n")
        absent_manifest = str(sb.root / "no-such-manifest.yaml")
        before = _tree_snapshot(repo)
        r = sb.run([sys.executable, str(repo / RENDER), "--topology", "single", "--manifest", absent_manifest], sb.root)
        ck("★S6 렌더 음성: 자기선언 multi 트리에서 --topology single → exit 5 · TOPOLOGY_RULES_MISMATCH · 스테이징 0",
           r.returncode == 5 and "TOPOLOGY_RULES_MISMATCH" in r.stderr and _tree_snapshot(repo) == before,
           (r.returncode, r.stderr[-400:]))
        r = sb.run([sys.executable, str(repo / RENDER), "--topology", "multi", "--manifest", absent_manifest], sb.root)
        ck("★S6 렌더 양성: 일치하면 가드를 지나 다음 검사(manifest 부재 exit 3)에 닿는다",
           r.returncode == 3 and "TOPOLOGY_RULES_MISMATCH" not in r.stderr, (r.returncode, r.stderr[-300:]))
        src = (repo / RENDER).read_text(encoding="utf-8")
        anchor = "    _mismatch = topology_rules_mismatch(args.topology)\n"
        ck("렌더 변이 앵커 1건", src.count(anchor) == 1, src.count(anchor))
        (repo / RENDER).write_text(src.replace(anchor, "    _mismatch = None\n"), encoding="utf-8")
        r = sb.run([sys.executable, str(repo / RENDER), "--topology", "single", "--manifest", absent_manifest], sb.root)
        ck("★S6 렌더 변이 음성대조: 대조를 끄면 반대 통로 렌더가 manifest 검사까지 간다(결함 재현)",
           r.returncode == 3, (r.returncode, r.stderr[-300:]))


# ════════════════════════════════════════════════════════════════════════════════════════════════
# S1 · S3 · S4 · S7 -- closing_sequence execute (multi-node → single-node)
# ════════════════════════════════════════════════════════════════════════════════════════════════

def _relocation_replacements() -> list[str]:
    src = (REAL / SYNC_BRANCHES).read_text(encoding="utf-8")
    m = re.search(r"^ROOT_RELOCATION_REPLACEMENTS=\((.*?)^\)", src, re.S | re.M)
    return [ln.split("#", 1)[0].strip() for ln in (m.group(1).splitlines() if m else []) if ln.split("#", 1)[0].strip()]


ENTRY_UTC = "2026-09-14T00:00:00Z"
APPROVED_AT = "2026-09-14T00:01:00Z"


def _build_sequence_fixture(sb: Sandbox, *, target_only_change: bool = False, broken_judge: bool = False,
                            carry_target_change: bool = False, forged_target_ledger: bool = False) -> dict:
    """출발 multi-node · 대상 single-node. 과거 동기화 1회(원장 E1 + single 착지)가 끝난 뒤 multi 가 공통층을 더 고친 상태.

    `carry_target_change` = 대상 전용 변경을 처방대로 출발 브랜치로 **옮긴** 상태(다른 출발 커밋 뒤에 같은 바이트).
    `forged_target_ledger` = 대상 브랜치가 같은 entry_id 를 다른 내용으로 든 원장을 추적한다(append-only 파손).
    """
    repo = sb.root / "repo"
    repo.mkdir()
    sb.git(repo, "init", "-q", "-b", "multi-node")
    reps = _relocation_replacements()
    if len(reps) < 10:
        raise RuntimeError(f"ROOT_RELOCATION_REPLACEMENTS 를 읽지 못했다: {reps}")
    for rel in {LAYER_LEDGER, SYNC_BRANCHES, CLOSING, PARITY, GATE, CATALOG, ".gitignore",
                *(f".claude/schemas/{s}" for s in SCHEMAS), *reps}:
        _copy_real(repo, rel)
    _write(repo, "CLAUDE.md", "# 픽스처 헌법 v1\n")
    _write(repo, ".claude/rules/common.md", "공통층 v1\n")
    _write(repo, "docs/plan/example.md", "# plan 스켈레톤 v1\n")
    _write(repo, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: multi** · layer: topology\n")
    # 실물 두 브랜치는 hints/ 추적 파일(계약서·families)을 든다 -- 없으면 ⑤ 의 중앙권위 마커 배치 자리가 없어
    # 픽스처가 실물보다 좁아진다(첫 실행에서 `cp: cannot create` 로 드러났다).
    _write(repo, "hints/FIXTURE_CONTRACT.md", "# hint 계약서 픽스처\n")
    sb.git(repo, "add", "-A")
    sb.git(repo, "commit", "-qm", "A")
    sb.git(repo, "branch", "single-node")

    _write(repo, "CLAUDE.md", "# 픽스처 헌법 v2\n")
    sb.git(repo, "commit", "-qam", "B 공통층")
    b = sb.git(repo, "rev-parse", "HEAD")
    e1 = {"entry_id": "sync-1", "utc": "2026-09-12T06:16:53Z", "departure": "multi-node", "departure_commit": b,
          "approved_by": "selftest", "approved_utc": "2026-09-12T06:20:00Z",
          "classification": [{"file": "CLAUDE.md", "verdict": "common_promote", "reason": "픽스처",
                              "provenance": "agent-judged"}]}
    _write(repo, LEDGER_REL, json.dumps({"schema_version": 1, "entries": [e1]}, ensure_ascii=False, indent=2) + "\n")
    sb.git(repo, "add", "-A")
    sb.git(repo, "commit", "-qm", "ledger(sync-1)")

    # 과거 착지: single-node 워크트리에서 B 트리를 공통층에 내려앉힌다(특화층은 single 그대로 · 원장은 옮기지 않는다)
    wt = sb.root / "prior-landing"
    sb.git(repo, "worktree", "add", "-q", str(wt), "single-node")
    sb.git(wt, "checkout", b, "--", ".")
    _write(wt, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: single** · layer: topology\n")
    sb.git(wt, "add", "-A")
    sb.git(wt, "commit", "-qm", "sync: 공통층 multi-node→single-node (과거 회차)")
    if target_only_change:
        _write(wt, ".claude/rules/common.md", "single 에서만 고친 공통층\n")
        sb.git(wt, "commit", "-qam", "single 전용 공통층 변경")
    if forged_target_ledger:
        forged = dict(e1, approved_by="someone-else")
        _write(wt, LEDGER_REL, json.dumps({"schema_version": 1, "entries": [forged]}, ensure_ascii=False) + "\n")
        sb.git(wt, "add", "-A")
        sb.git(wt, "commit", "-qm", "대상 원장(같은 id · 다른 내용)")
    sb.git(repo, "worktree", "remove", "--force", str(wt))

    # 이번 회차가 옮길 것: 공통층 · docs 스켈레톤 · 판정기 자신(대상의 판정기는 옛 판본이 된다)
    _write(repo, "CLAUDE.md", "# 픽스처 헌법 v3\n")
    _write(repo, "docs/plan/example.md", "# plan 스켈레톤 v2\n")
    # 인증서(복사 전용 arm · 대상 같은 경로를 충돌 검사 없이 덮는다) -- 분류표 빈칸에 보여야 한다(갈라짐 판정에서는 빠진다)
    _write(repo, CERT_REL, "fixture: 재현성 인증서\n")
    sb.git(repo, "add", "-f", CERT_REL)
    with (repo / LAYER_LEDGER).open("a", encoding="utf-8") as fh:
        fh.write("\n# fixture: 판정기 개정 회차(대상 브랜치의 판정기는 옛 판본이다)\n")
    if broken_judge:
        # 판정기 **자체**가 실패하는 회차(분류·원장 경로는 멀쩡하다) -- 판정하지 못한 덮어쓰기가 통과하는지 본다
        judge = (repo / LAYER_LEDGER).read_text(encoding="utf-8")
        anchor = "    sp = None\n    base = since\n"       # divergence() 첫 코드 줄(자체검사가 1건임을 확인한다)
        if judge.count(anchor) != 1:
            raise RuntimeError("판정기 고장 주입 앵커를 찾지 못했다")
        (repo / LAYER_LEDGER).write_text(
            judge.replace(anchor, '    raise RuntimeError("fixture: 판정기 고장 주입")\n' + anchor), encoding="utf-8")
    sb.git(repo, "commit", "-qam", "C 공통층·스켈레톤·판정기")
    if carry_target_change:
        # 처방("위 커밋을 출발 브랜치로 먼저 옮기세요") 그대로: 다른 출발 커밋(C) 뒤에 대상의 바이트를 받는다
        _write(repo, ".claude/rules/common.md", "single 에서만 고친 공통층\n")
        sb.git(repo, "commit", "-qam", "cherry-pick: single 전용 공통층 변경을 출발 브랜치로 옮김")
    head = sb.git(repo, "rev-parse", "HEAD")

    # 원격: 로컬에 없는 hint 태그 1건을 든 bare 저장소(카탈로그 파생의 진실원천)
    remote = sb.root / "remote.git"
    sb.git(sb.root, "init", "-q", "--bare", str(remote))
    other = sb.root / "other-pc"
    sb.git(sb.root, "clone", "-q", str(remote), str(other))
    _write(other, "payload.txt", "원격 전용\n")
    sb.git(other, "add", "-A")
    sb.git(other, "commit", "-qm", "remote-only payload")
    sb.git(other, "tag", "-a", "hint/9.9.9/fixture-model/gb10-main-x/qfp8", "-m", "원격 전용 brief")
    sb.git(other, "push", "-q", "origin", "HEAD:refs/heads/hint", "refs/tags/hint/9.9.9/fixture-model/gb10-main-x/qfp8")
    sb.git(repo, "remote", "add", "fx", str(remote))
    (repo / "hints").mkdir(exist_ok=True)
    (repo / "hints/.central_authority").write_text("", encoding="utf-8")

    # 승인 증거(비추적 · 게이트는 repo-root 아래에서만 해소한다)
    atoms = ["approved_by: selftest", f"approved_at_utc: {APPROVED_AT}", "allowed_action: sync_branches"]
    plan = repo / "docs/plan/p_selftest.md"
    plan.write_text("# selftest plan\n\n## Execution approval\n\n" + "\n".join(atoms) + "\n", encoding="utf-8")
    manifest = repo / "docs/_evidence/wm_selftest.json"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps({
        "schema_version": 1, "task_class": "harness_change",
        "identity": {"model": "m", "gpu": "g", "vllm": "v", "quant": None, "topology": "multi", "tp": 1},
        "evidence": {"plan": {"path": "../plan/p_selftest.md"}}, "pii_scan": {"passed": True},
        "execution_approval": {"approved": True, "approved_by": "selftest", "approved_at_utc": APPROVED_AT,
                               "plan_path": "../plan/p_selftest.md",
                               "plan_sha256": hashlib.sha256(plan.read_bytes()).hexdigest(),
                               "approval_anchor": "## Execution approval", "approval_atoms": atoms,
                               "allowed_actions": ["sync_branches"]}}, ensure_ascii=False), encoding="utf-8")
    ck("픽스처 전제: 대상 브랜치에 원장이 없다(S1 의 형태) · 위조 원장 픽스처만 예외",
       (subprocess.run(["git", "-C", str(repo), "cat-file", "-e", f"single-node:{LEDGER_REL}"],
                       capture_output=True, env=sb.env).returncode != 0) != forged_target_ledger)
    ck("픽스처 전제: 대상 브랜치의 판정기가 출발 브랜치와 다르다(옛 판본)",
       sb.git(repo, "rev-parse", f"single-node:{LAYER_LEDGER}") != sb.git(repo, "rev-parse", f"multi-node:{LAYER_LEDGER}"))
    return {"repo": repo, "manifest": manifest, "head": head, "e1": e1}


def _entry(fx: dict, **over) -> dict:
    e = {"entry_id": "sync-2", "utc": ENTRY_UTC, "departure": "multi-node", "departure_commit": fx["head"],
         "approved_by": "selftest", "approved_utc": APPROVED_AT,
         "classification": [{"file": f, "verdict": "common_promote", "reason": "픽스처 공통층",
                             "provenance": "agent-judged"}
                            for f in ("CLAUDE.md", "docs/plan/example.md", LAYER_LEDGER)]}
    e.update(over)
    return e


def _execute(sb: Sandbox, fx: dict, entry: dict, script: Path | None = None) -> subprocess.CompletedProcess:
    entry_file = sb.root / "entry.json"
    entry_file.write_text(json.dumps(entry, ensure_ascii=False, indent=2), encoding="utf-8")
    return sb.run(["bash", str(script or (fx["repo"] / CLOSING)), "execute", "--repo", str(fx["repo"]),
                   "--entry-file", str(entry_file), "--mode", "experimental", "--manifest", str(fx["manifest"]),
                   "--remote", "fx", "--no-sub", "--no-push"], cwd=sb.root)


def test_closing_sequence() -> None:
    print("[S1·S3·S4·S7] closing_sequence execute multi-node → single-node", file=sys.stderr)

    # ── S4 · S3: 빈칸은 원장 소유자의 자리표시자로 만들어지고, 그대로 내면 어떤 부작용보다 먼저 거부된다 ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb)
        repo = fx["repo"]
        r = sb.run(["bash", str(repo / CLOSING), "preflight", "--repo", str(repo)], sb.root)
        marker = "② 분류표 빈칸"
        skeleton = None
        if marker in r.stdout:
            try:
                skeleton = json.loads(r.stdout.split(marker, 1)[1].split("\n", 1)[1])
            except ValueError:
                skeleton = None
        files = [row.get("file") for row in (skeleton or {}).get("classification") or []]
        ck("preflight 가 분류표 빈칸 JSON 을 낸다", r.returncode == 0 and isinstance(skeleton, dict),
           (r.returncode, r.stdout[-500:], r.stderr[-500:]))
        ck("★S3 빈칸이 docs/*/example.md 변경을 보여 준다(종전 pathspec 은 docs/benchmark 만 붙였다)",
           "docs/plan/example.md" in files and "CLAUDE.md" in files and TOPOLOGY_RULES not in files, files)
        ck("★S3 빈칸이 충돌 검사 없이 덮는 인증서(복사 전용 arm)를 보여 준다 · report 는 뺀다",
           CERT_REL in files and not any(f.startswith("docs/report/") for f in files), files)
        sk_file = sb.root / "skeleton.json"
        sk_file.write_text(json.dumps(skeleton or {}, ensure_ascii=False), encoding="utf-8")
        v = sb.run([sys.executable, str(repo / LAYER_LEDGER), "validate", "--entry-file", str(sk_file)], sb.root)
        try:
            vdoc = json.loads(v.stdout)
        except ValueError:
            vdoc = {}
        ck("★S4 빈칸 생성기의 자리표시자 = 원장이 거부하는 자리표시자(같은 소유자)",
           v.returncode == 5 and sum("자리표시자" in p for p in vdoc.get("problems") or []) >= 6, vdoc)

        head_before = sb.git(repo, "rev-parse", "multi-node")
        single_before = sb.git(repo, "rev-parse", "single-node")
        ledger_before = (repo / LEDGER_REL).read_bytes()
        r = _execute(sb, fx, dict(skeleton or {}))
        # 종료 시퀀스 자기 FAIL 문장에도 "자리표시자" 가 들어 있다 -- **검사기의 항목 문장**과 필드 경로를 본다.
        ck("★S4 execute: 빈칸 그대로의 분류표 → exit 2 · 원장·커밋·워크트리 부작용 0",
           r.returncode == 2 and "자리표시자가 남아 있다" in r.stderr and "approved_utc" in r.stderr
           and sb.git(repo, "rev-parse", "multi-node") == head_before
           and sb.git(repo, "rev-parse", "single-node") == single_before
           and (repo / LEDGER_REL).read_bytes() == ledger_before
           and len(sb.git(repo, "worktree", "list", "--porcelain").split("worktree ")) == 2,
           (r.returncode, r.stderr[-500:]))
        r = _execute(sb, fx, _entry(fx, approved_by="<FILL: 사람>"))
        ck("★S4 execute: 한 칸만 남아도 거부(approved_by 자리표시자 · 검사기 항목 문장으로 확인)",
           r.returncode == 2 and "자리표시자가 남아 있다" in r.stderr and ": approved_by --" in r.stderr,
           (r.returncode, r.stderr[-300:]))
        r = _execute(sb, fx, _entry(fx, approved_by=""))
        ck("S4 음성대조: 빈 필드 거부는 자리표시자 문장이 아니다(원인이 섞이지 않는다)",
           r.returncode == 2 and "필수 필드 없음: approved_by" in r.stderr and "자리표시자가 남아 있다" not in r.stderr,
           (r.returncode, r.stderr[-300:]))

    # ── S1 · S7 정상 경로: 대상에 원장 없음 · 판정기 옛 판본 · 원격 전용 태그 ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb)
        repo = fx["repo"]
        r = _execute(sb, fx, _entry(fx))
        out = r.stdout + r.stderr
        ck("★S1 closing execute(multi→single) 완주 rc=0", r.returncode == 0, (r.returncode, out[-1500:]))
        ck("★S1 RED ② 가 대상에 원장이 없어도 실제로 판정한다: clean · basis=ledger-sync-point · 동기 지점=converged",
           "공통층 갈라짐 검사: clean" in out and "basis=ledger-sync-point" in out and "동기 지점=converged" in out
           and "판정 불가" not in out, out[-1500:])
        # 파생기 자신의 die 문장이 아니라 **종료 시퀀스의 원인 분류 줄**을 본다(파생기 문장만으로도 fetch 명령은 보이므로).
        ck("★S7 종료 시퀀스가 원인을 분류한다: 로컬 태그 오브젝트 부재 · 그 원격(fx)의 fetch 명령 · 기재하고 진행",
           "원인: 원격 fx 에만 있는 hint 태그의 **로컬 태그 오브젝트 부재**" in out
           and "[closing]     해소(사람 · 자동 fetch 하지 않는다): git -C" in out
           and "fetch fx 'refs/tags/hint/*:refs/tags/hint/*'" in out
           and "카탈로그는 이번 회차에 갱신되지 않았다" in out, out[-1500:])
        ck("★S7 파생기 안내도 곧 지워질 임시 워크트리가 아니라 주 워크트리를 가리킨다(두 안내가 같은 저장소)",
           ".wt-single-node fetch" not in out and f"git -C {repo} fetch fx" in out, out[-1500:])
        ck("★S7 자동 fetch 하지 않는다(원격 전용 태그는 여전히 로컬에 없다)",
           subprocess.run(["git", "-C", str(repo), "rev-parse", "--verify", "--quiet",
                           "refs/tags/hint/9.9.9/fixture-model/gb10-main-x/qfp8"],
                          capture_output=True, env=sb.env).returncode != 0)
        ck("동기화가 공통층·docs 스켈레톤·판정기를 옮겼다 · 특화층은 옮기지 않았다",
           sb.git(repo, "show", "single-node:CLAUDE.md") == "# 픽스처 헌법 v3"
           and sb.git(repo, "show", "single-node:docs/plan/example.md") == "# plan 스켈레톤 v2"
           and sb.git(repo, "rev-parse", f"single-node:{LAYER_LEDGER}") == sb.git(repo, "rev-parse", f"multi-node:{LAYER_LEDGER}")
           and "**topology: single**" in sb.git(repo, "show", f"single-node:{TOPOLOGY_RULES}"))
        led = json.loads(sb.git(repo, "show", f"multi-node:{LEDGER_REL}"))
        ck("원장: 과거 항목 불변(append-only) · 이번 항목 추가 · 대상 브랜치로 복제 안 됨",
           led["entries"][0] == fx["e1"] and [e["entry_id"] for e in led["entries"]] == ["sync-1", "sync-2"]
           and subprocess.run(["git", "-C", str(repo), "cat-file", "-e", f"single-node:{LEDGER_REL}"],
                              capture_output=True, env=sb.env).returncode != 0, led)
        ck("잔존 워크트리 0", len(sb.git(repo, "worktree", "list", "--porcelain").split("worktree ")) == 2)

    # ── S1 음성대조 ①: 대상 전용 공통층 변경 → diverged 로 멈추고 대상 브랜치를 건드리지 않는다 ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb, target_only_change=True)
        repo = fx["repo"]
        single_before = sb.git(repo, "rev-parse", "single-node")
        r = _execute(sb, fx, _entry(fx))
        out = r.stdout + r.stderr
        ck("★S1 음성대조: 대상 전용 공통층 변경 → COMMON_LAYER_DIVERGED · closing rc=5 · 대상 브랜치 불변",
           r.returncode == 5 and "COMMON_LAYER_DIVERGED" in out and ".claude/rules/common.md" in out
           and sb.git(repo, "rev-parse", "single-node") == single_before
           and len(sb.git(repo, "worktree", "list", "--porcelain").split("worktree ")) == 2,
           (r.returncode, out[-1200:]))

    # ── 교착 해소: 처방대로 대상 전용 변경을 출발 브랜치로 옮기면 같은 시퀀스가 **완주**한다(위 음성대조 ①의 짝) ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb, target_only_change=True, carry_target_change=True)
        repo = fx["repo"]
        r = _execute(sb, fx, _entry(fx))
        out = r.stdout + r.stderr
        ck("★교착 해소: 대상 전용 변경을 출발 브랜치로 옮긴 뒤(다른 출발 커밋 C 뒤) execute → rc=0 · clean · 흡수 1건",
           r.returncode == 0 and "공통층 갈라짐 검사: clean" in out and "흡수 1건" in out
           and "COMMON_LAYER_DIVERGED" not in out, (r.returncode, out[-1500:]))
        ck("교착 해소: 옮긴 바이트가 대상에 그대로 남는다(잃은 것 없음)",
           sb.git(repo, "show", "single-node:.claude/rules/common.md") == "single 에서만 고친 공통층")

    # ── 원장 무결성: 대상이 같은 id 다른 내용의 원장을 들면 약한 판정으로 접지 않고 멈춘다(COMMON_LAYER_UNJUDGED) ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb, forged_target_ledger=True)
        repo = fx["repo"]
        single_before = sb.git(repo, "rev-parse", "single-node")
        r = _execute(sb, fx, _entry(fx))
        out = r.stdout + r.stderr
        ck("★원장 충돌 → COMMON_LAYER_UNJUDGED · basis=ledger-integrity · closing rc=5 · 대상 브랜치 불변",
           r.returncode == 5 and "COMMON_LAYER_UNJUDGED" in out and "ledger-integrity" in out
           and "ledger-conflict" in out and sb.git(repo, "rev-parse", "single-node") == single_before,
           (r.returncode, out[-1200:]))

    # ── 출발 브랜치에 판정기가 없으면(도입 이전 브랜치) 경고 뒤 덮지 않고 멈춘다 ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb)
        repo = fx["repo"]
        sb.git(repo, "branch", "legacy-no-judge", "multi-node")
        lg = sb.root / "legacy"
        sb.git(repo, "worktree", "add", "-q", str(lg), "legacy-no-judge")
        sb.git(lg, "rm", "-q", LAYER_LEDGER)
        sb.git(lg, "commit", "-qm", "판정기 도입 이전 형태")
        sb.git(repo, "worktree", "remove", "--force", str(lg))
        wt = sb.root / "dst"
        sb.git(repo, "worktree", "add", "-q", str(wt), "single-node")
        sb.git(wt, "checkout", "legacy-no-judge", "--", SYNC_BRANCHES)
        for rel in ("docs/_evidence/wm_selftest.json", "docs/plan/p_selftest.md"):
            (wt / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(repo / rel, wt / rel)
        single_before = sb.git(repo, "rev-parse", "single-node")
        r = sb.run(["bash", str(wt / SYNC_BRANCHES), "--from", "legacy-no-judge", "--mode", "experimental",
                    "--manifest", "docs/_evidence/wm_selftest.json"], cwd=wt)
        ck("★출발 브랜치에 판정기 없음 → COMMON_LAYER_UNJUDGED exit 10(판정 없는 덮어쓰기 ✗) · 대상 불변",
           r.returncode == 10 and "COMMON_LAYER_UNJUDGED" in r.stderr and "판정기" in r.stderr
           and sb.git(repo, "rev-parse", "single-node") == single_before, (r.returncode, r.stdout[-400:], r.stderr[-600:]))

    # ── S1 음성대조 ③: 판정기 자체가 실패하면 판정 불가를 통과로 두지 않고 멈춘다(COMMON_LAYER_UNJUDGED) ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb, broken_judge=True)
        repo = fx["repo"]
        single_before = sb.git(repo, "rev-parse", "single-node")
        r = _execute(sb, fx, _entry(fx))
        out = r.stdout + r.stderr
        ck("★S1 음성대조: 판정기 실패 → COMMON_LAYER_UNJUDGED · closing rc=5 · 대상 브랜치 불변(판정 못 한 덮어쓰기 ✗)",
           r.returncode == 5 and "COMMON_LAYER_UNJUDGED" in out and "판정기 고장 주입" in out
           and sb.git(repo, "rev-parse", "single-node") == single_before, (r.returncode, out[-1000:]))

    # ── S1 음성대조 ②: 판정기 선행 당김을 걷어낸 종료 시퀀스 → 대상의 옛 판정기로 판정하지 않고 멈춘다 ──
    with tempfile.TemporaryDirectory() as td:
        sb = Sandbox(Path(td).resolve())
        fx = _build_sequence_fixture(sb)
        repo = fx["repo"]
        src = (repo / CLOSING).read_text(encoding="utf-8")
        anchor = "sync_branches.sh \\\n" + PREPULL_JUDGE
        ck("종료 시퀀스 변이 앵커(판정기 선행 당김) 1건", src.count(anchor) == 1, src.count(anchor))
        mutated = sb.root / "closing_mutated.sh"
        mutated.write_text(src.replace(anchor, "sync_branches.sh\n"), encoding="utf-8")
        single_before = sb.git(repo, "rev-parse", "single-node")
        r = _execute(sb, fx, _entry(fx), script=mutated)
        out = r.stdout + r.stderr
        ck("★S1 변이 음성대조: 판정기를 당기지 않으면 LEDGER_JUDGE_STALE 로 멈춘다(옛 규칙 판정 ✗) · 대상 불변",
           r.returncode == 5 and "LEDGER_JUDGE_STALE" in out and sb.git(repo, "rev-parse", "single-node") == single_before,
           (r.returncode, out[-1000:]))


def main() -> int:
    for rel in (SYNC_TO_SUB, SYNC_BRANCHES, LAYER_LEDGER, CLOSING, PARITY, RENDER, GATE, CATALOG):
        if not (REAL / rel).is_file():
            print(f"[selftest_branch_sync] FAIL: 검사 대상이 없다: {rel}", file=sys.stderr)
            return 1
    for test in (test_sync_to_sub_guard, test_render_guard, test_closing_sequence):
        try:
            test()
        except Exception as exc:  # noqa: BLE001 -- 픽스처 구성 실패도 FAIL 로 센다(침묵 통과 ✗)
            FAILURES.append(f"{test.__name__}: {type(exc).__name__}: {exc}"[:1500])
            print(f"  FAIL {test.__name__} 예외: {exc}", file=sys.stderr)
    if FAILURES:
        for f in FAILURES:
            print(f"[selftest_branch_sync] FAIL: {f}", file=sys.stderr)
        return 1
    print("[selftest_branch_sync] PASS", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
