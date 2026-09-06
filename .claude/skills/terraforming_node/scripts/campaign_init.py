#!/usr/bin/env python3
"""campaign_init.py — 캠페인 인스턴스의 개설·경로 파생·purge 게이트 (fail-closed).

세 가지 일을 한다.

1. **경로 파생** (`--derive`): producer 가 자기 출력 자리를 활성 캠페인 선언에서 얻는다.
   루트 기본값을 남기지 않는 것이 이 스크립트의 존재 이유다 — 남기면 선언을 잊은 실행이 조용히
   루트에 쓴다(2026-09-06 실측: 그렇게 21개가 공개 원격까지 갔다).
2. **purge 게이트** (`--verify-purge-gate`): 직전 인스턴스를 지워도 되는가. 증거가 docs 평면에
   실제로 도착했을 때만 열린다. 열리지 않으면 새 캠페인이 **시작되지 않는다** — 증거를 흘린 채
   다음 캠페인을 도는 것보다 멈추는 편이 싸다.
3. **개설** (`--init ... --apply`): purge → 뼈대 복사 → 활성 포인터 기록.

무결성 해시는 두지 않는다(`policy:GIT_SINGLE_AUTHORITY` · 사용자 결정 2026-09-06).
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
CAMPAIGNS = REPO_ROOT / "campaigns"
TEMPLATE = CAMPAIGNS / "_template"
ACTIVE_POINTER = CAMPAIGNS / "ACTIVE"      # 비추적(휘발) — `/campaigns/*` 가 이미 무시한다
BOOTSTRAP = "_bootstrap"
RESERVED_IDS = ("_template", BOOTSTRAP)
# purge 가 손대지 않는 것. sync_staging 은 루트 상시 자원이라 애초에 이 트리 밖이지만, 명시해 둔다 —
# "왜 안 지웠나" 를 나중에 사람이 묻지 않게 하는 것도 게이트의 일이다.
PURGE_KEEPS = ("_template", "README.md", "ACTIVE")


class PurgeGateRefusal(Exception):
    """게이트가 닫혀 있다. 우회하지 말고 사유를 해소한다(D3)."""


def _rel(path: Path) -> str:
    """저장소 안이면 상대경로, 밖(픽스처)이면 그대로 — 표시 편의가 판정을 죽이지 않게."""
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def _read_json(path: Path) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def campaigns_dir(repo_root: str | Path | None = None) -> Path:
    """캠페인 워크스페이스 루트. `repo_root` 를 받는 이유: 서브 워크트리·픽스처에서도 같은 규칙이
    돌아야 하는데, 모듈 전역만 있으면 그 자리에서 규칙이 두 벌로 갈라진다."""
    return CAMPAIGNS if repo_root is None else Path(repo_root) / "campaigns"


def active_campaign_id(repo_root: str | Path | None = None) -> str:
    """활성 캠페인 id. 포인터가 없으면 예약 id `_bootstrap` 이다 — **루트가 아니다**.

    부재를 루트 폴백으로 처리하지 않는 것이 핵심이다. 폴백이 루트였기 때문에 선언을 잊은 실행이
    조용히 루트에 썼다(침묵 폴백 금지 · workflow.md §4종 안티패턴 판정표 '결함' 칸).
    """
    base = campaigns_dir(repo_root)
    pointer = ACTIVE_POINTER if repo_root is None else base / "ACTIVE"
    if pointer.is_file():
        name = pointer.read_text(encoding="utf-8").strip()
        if name and (base / name).is_dir():
            return name
    return BOOTSTRAP


def derive_path(kind: str, *, cell: str | None = None, sweep: str | None = None,
                context: str | None = None, camp_id: str | None = None,
                repo_root: str | Path | None = None) -> Path:
    """producer 출력 경로. 이 함수가 producer 경로의 **단일 소유자**다."""
    camp = camp_id or active_campaign_id(repo_root)
    base = campaigns_dir(repo_root) / camp
    if kind == "root":
        return base
    if kind in ("config", "lockset", "cell-status"):
        if not cell:
            raise PurgeGateRefusal(f"--derive {kind} 는 --cell 이 필요하다(셀 없는 셀 입력은 없다)")
        name = {"config": "config.yaml", "lockset": "lockset.json",
                "cell-status": "cell.status.json"}[kind]
        return base / "cells" / cell / name
    if kind == "sweep":
        if not sweep:
            raise PurgeGateRefusal("--derive sweep 은 --sweep 이 필요하다")
        return base / "sweeps" / f"{sweep}.json"
    if kind == "relay":
        return base / "relay" / (f"{context}.json" if context else "")
    if kind == "relay-root":
        return base / "relay"
    if kind == "pending-hitl":
        return base / "relay" / "pending_hitl.json"
    if kind == "evidence":
        return base / "evidence_pointers.json"
    if kind == "phases":
        return base / "phases"
    raise PurgeGateRefusal(f"알 수 없는 파생 종류: {kind!r}")


def purge_gate_reasons(previous_id: str) -> list[str]:
    """직전 인스턴스를 지울 수 없는 **사유 목록**(빈 리스트 = 게이트 열림)."""
    reasons: list[str] = []
    prev = CAMPAIGNS / previous_id
    if not prev.is_dir():
        return reasons  # 지울 것이 없으면 게이트는 자명하게 열려 있다

    pointers_path = prev / "evidence_pointers.json"
    doc = _read_json(pointers_path)
    if doc is None:
        reasons.append(f"{previous_id}: evidence_pointers.json 이 없거나 파손 — 증거가 docs 평면에 "
                       f"도착했는지 물을 수단이 없다")
        return reasons

    pointers = doc.get("pointers") if isinstance(doc, dict) else None
    if not isinstance(pointers, list) or not pointers:
        reasons.append(f"{previous_id}: 증거 포인터가 0건이다 — 캠페인을 돌고 증거가 하나도 없다는 "
                       f"기록은 그 자체가 결손이다(빈 목록으로 게이트를 통과시키지 않는다)")
        return reasons

    kinds = set()
    for i, ptr in enumerate(pointers):
        if not isinstance(ptr, dict):
            reasons.append(f"{previous_id}: pointers[{i}] 형태 위반")
            continue
        rel = str(ptr.get("path") or "").strip()
        kind = str(ptr.get("kind") or "").strip()
        kinds.add(kind)
        if not rel or "<<FILL>>" in rel:
            reasons.append(f"{previous_id}: pointers[{i}] 경로 미기재")
            continue
        if not (REPO_ROOT / rel).exists():
            reasons.append(f"{previous_id}: 증거가 실재하지 않는다 — {rel} (kind={kind})")
    # 릴레이 요약은 원장 원문이 휘발이므로 **서사가 남았는지**를 따로 묻는다.
    if "relay_summary" not in kinds:
        reasons.append(f"{previous_id}: relay_summary 포인터가 없다 — 원장 원문은 휘발이므로 "
                       f"요약 testlog 가 없으면 왕복이 통째로 사라진다")
    return reasons


def residue_scan() -> dict:
    """잔재 스캔. purge 완료 조건은 leak·unclassified 가 0 인 것이다."""
    out = {"workspace": [], "evidence": [], "volatile": [], "leak": [], "unclassified": []}
    registry = _read_json(REPO_ROOT / ".claude/policies/root_registry.json") or {}
    declared = {e.get("name") for e in registry.get("entries", []) if isinstance(e, dict)}
    tombs = [t.get("name") for t in registry.get("tombstones", []) if isinstance(t, dict)]
    import fnmatch
    for child in sorted(REPO_ROOT.iterdir()):
        name = child.name
        if name in declared:
            out["workspace"].append(name)
            continue
        if any(fnmatch.fnmatch(name, pat) for pat in tombs if pat):
            out["leak"].append(name)
            continue
        out["unclassified"].append(name)
    for camp in sorted(p for p in CAMPAIGNS.iterdir() if p.is_dir()) if CAMPAIGNS.is_dir() else []:
        if camp.name in ("_template",):
            continue
        out["volatile"].append(f"campaigns/{camp.name}")
    return out


def do_purge(previous_id: str, *, apply: bool) -> list[str]:
    reasons = purge_gate_reasons(previous_id)
    if reasons:
        raise PurgeGateRefusal("purge 게이트가 닫혀 있다:\n  - " + "\n  - ".join(reasons))
    prev = CAMPAIGNS / previous_id
    if not prev.is_dir():
        return []
    if apply:
        shutil.rmtree(prev)
    return [_rel(prev)]


def scaffold(camp_id: str, plan_ref: str, *, apply: bool) -> list[str]:
    if camp_id in RESERVED_IDS:
        raise PurgeGateRefusal(f"예약 id 로는 캠페인을 열 수 없다: {camp_id!r}")
    dest = CAMPAIGNS / camp_id
    if dest.exists():
        raise PurgeGateRefusal(f"이미 있는 인스턴스다: campaigns/{camp_id} (먼저 purge 하라)")
    made: list[str] = []
    if apply:
        shutil.copytree(TEMPLATE, dest)
        # `_cell`·`_node` 는 **틀**이다. 실제 셀·노드가 생기기 전까지 그대로 두면 검증기가
        # 빈칸 잔존으로 잡는다 — 그것이 의도다(빈칸이 곧 계약).
        doc = json.loads((dest / "campaign.yaml").read_text(encoding="utf-8"))
        doc["id"] = camp_id
        doc["plan_ref"] = plan_ref
        (dest / "campaign.yaml").write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8")
        ACTIVE_POINTER.write_text(camp_id + "\n", encoding="utf-8")
    made.append(f"campaigns/{camp_id}")
    return made


def _selftest() -> int:
    import tempfile
    global CAMPAIGNS, ACTIVE_POINTER
    ok = True

    def ck(label, cond):
        nonlocal ok
        print(("  PASS " if cond else "  FAIL ") + label)
        ok = ok and bool(cond)

    ck("활성 부재는 _bootstrap 이지 루트가 아니다", active_campaign_id() in (BOOTSTRAP,) or (CAMPAIGNS / active_campaign_id()).is_dir())
    ck("파생 config 는 campaigns 아래",
       "campaigns/" in str(derive_path("config", cell="c1", camp_id="x")) and str(derive_path("config", cell="c1", camp_id="x")).endswith("cells/c1/config.yaml"))
    ck("파생 relay 루트는 tasks 가 아니다", "tasks" not in str(derive_path("relay-root", camp_id="x")))
    try:
        derive_path("config", camp_id="x"); cell_guard = False
    except PurgeGateRefusal:
        cell_guard = True
    ck("셀 없는 셀 입력 차단", cell_guard)
    try:
        derive_path("nope"); kind_guard = False
    except PurgeGateRefusal:
        kind_guard = True
    ck("알 수 없는 파생 차단", kind_guard)

    saved = (CAMPAIGNS, ACTIVE_POINTER)
    with tempfile.TemporaryDirectory() as tmp:
        CAMPAIGNS = Path(tmp) / "campaigns"
        ACTIVE_POINTER = CAMPAIGNS / "ACTIVE"
        prev = CAMPAIGNS / "old"
        prev.mkdir(parents=True)
        ck("포인터 파일 부재 → 게이트 닫힘", bool(purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps({"pointers": []}), encoding="utf-8")
        ck("포인터 0건 → 게이트 닫힘", any("0건" in r for r in purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "does/not/exist.yaml"}]}), encoding="utf-8")
        ck("증거 부재 → 게이트 닫힘", any("실재하지 않는다" in r for r in purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"}]}), encoding="utf-8")
        ck("릴레이 요약 부재 → 게이트 닫힘", any("relay_summary" in r for r in purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"},
                          {"kind": "relay_summary", "path": "README.md"}]}), encoding="utf-8")
        ck("전수 실재 + 요약 → 게이트 열림", not purge_gate_reasons("old"))
        ck("dry-run 은 지우지 않는다", do_purge("old", apply=False) and prev.is_dir())
        ck("apply 는 지운다", do_purge("old", apply=True) is not None and not prev.is_dir())
        # 인스턴스가 둘 남았을 때(중단·누출 흡수) 정식 경로로 둘 다 지울 수 있는가.
        # 하나만 지워지면 남은 하나는 rm -rf 우회를 부른다.
        good = {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"},
                             {"kind": "relay_summary", "path": "README.md"}]}
        for _n in ("relicA", "relicB"):
            _d = CAMPAIGNS / _n
            _d.mkdir(parents=True)
            (_d / "evidence_pointers.json").write_text(json.dumps(good), encoding="utf-8")
        ck("게이트는 인스턴스마다 따로 묻는다",
           not purge_gate_reasons("relicA") and not purge_gate_reasons("relicB"))
        _removed = []
        for _n in ("relicA", "relicB"):
            _removed.extend(do_purge(_n, apply=True))
        ck("잔재 2건을 정식 경로로 전부 지운다",
           len(_removed) == 2 and not (CAMPAIGNS / "relicA").exists()
           and not (CAMPAIGNS / "relicB").exists())
        # 음성대조: 한쪽 게이트가 닫혀 있으면 그 하나만 거부되고 다른 하나는 영향받지 않는다.
        (CAMPAIGNS / "relicC").mkdir(parents=True)
        _refused = False
        try:
            do_purge("relicC", apply=True)
        except PurgeGateRefusal:
            _refused = True
        ck("★음성대조: 포인터 없는 잔재는 여전히 거부된다",
           _refused and (CAMPAIGNS / "relicC").is_dir())
    CAMPAIGNS, ACTIVE_POINTER = saved
    print("[campaign_init] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--active", action="store_true", help="활성 캠페인 id 를 출력")
    ap.add_argument("--derive", help="producer 출력 경로 파생 (root|config|lockset|cell-status|sweep|relay|relay-root|pending-hitl|evidence|phases)")
    ap.add_argument("--cell"); ap.add_argument("--sweep"); ap.add_argument("--context")
    ap.add_argument("--campaign-id", help="파생 대상 캠페인(생략 시 활성)")
    ap.add_argument("--verify-purge-gate", metavar="PREV_ID", help="직전 인스턴스의 purge 선행조건 검사")
    ap.add_argument("--residue-scan", action="store_true")
    ap.add_argument("--init", metavar="CAMP_ID", help="새 캠페인 개설")
    ap.add_argument("--plan-ref", help="--init 의 승인 plan 경로(필수)")
    # 반복 지정 가능(2026-09-06). 설계 의도는 "활성 인스턴스 1개"지만 현실은 그렇지 않을 수
    # 있다 — 중단된 캠페인·루트 누출 흡수분이 인스턴스를 둘 이상 남긴다. 단일 id 만 받으면
    # 남은 하나를 지울 정식 경로가 없어져 rm -rf 우회를 부른다(D3: 우회 대신 경로를 고친다).
    ap.add_argument("--purge-previous", metavar="PREV_ID", action="append", default=[],
                    help="--init 과 함께: 먼저 지울 직전 인스턴스(반복 지정 가능)")
    ap.add_argument("--apply", action="store_true", help="실제로 쓰고 지운다(기본은 dry-run)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.selftest:
            return _selftest()
        if a.active:
            print(active_campaign_id()); return 0
        if a.derive:
            print(derive_path(a.derive, cell=a.cell, sweep=a.sweep, context=a.context,
                              camp_id=a.campaign_id)); return 0
        if a.residue_scan:
            print(json.dumps(residue_scan(), ensure_ascii=False, indent=2)); return 0
        if a.verify_purge_gate:
            reasons = purge_gate_reasons(a.verify_purge_gate)
            if reasons:
                print("[campaign_init] purge 게이트 닫힘", file=sys.stderr)
                for r in reasons:
                    print(f"  - {r}", file=sys.stderr)
                return 1
            print("[campaign_init] purge 게이트 열림"); return 0
        if a.init:
            if not a.plan_ref:
                print("[campaign_init] FAIL --init 은 --plan-ref 가 필요하다(승인 없는 개설 ✗)", file=sys.stderr)
                return 2
            removed = []
            # 게이트는 **인스턴스마다** 따로 묻는다 — 하나가 열렸다고 다른 하나가 열리지 않는다.
            for _prev in a.purge_previous:
                removed.extend(do_purge(_prev, apply=a.apply))
            made = scaffold(a.init, a.plan_ref, apply=a.apply)
            mode = "APPLIED" if a.apply else "DRY-RUN"
            print(f"[campaign_init] {mode} purge={removed} init={made}")
            return 0
        ap.print_help(); return 2
    except PurgeGateRefusal as exc:
        print(f"[campaign_init] FAIL {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
