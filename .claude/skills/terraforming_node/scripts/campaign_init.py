#!/usr/bin/env python3
"""campaign_init.py — 캠페인 인스턴스의 개설·경로 파생·purge 게이트 (fail-closed).

네 가지 일을 한다.

1. **경로 파생** (`--derive`): producer 가 자기 출력 자리를 활성 캠페인 선언에서 얻는다.
   루트 기본값을 남기지 않는 것이 이 스크립트의 존재 이유다 — 남기면 선언을 잊은 실행이 조용히
   루트에 쓴다(2026-09-06 실측: 그렇게 21개가 공개 원격까지 갔다).
2. **purge 게이트** (`--verify-purge-gate`): 직전 인스턴스를 지워도 되는가. 증거가 docs 평면에
   실제로 도착했을 때만 열린다. 열리지 않으면 새 캠페인이 **시작되지 않는다** — 증거를 흘린 채
   다음 캠페인을 도는 것보다 멈추는 편이 싸다.
3. **개설** (`--init ... --apply`): purge → 뼈대 복사 → 활성 포인터 기록.
4. **단일 writer** (`--phase-set`·`--cell-set`·`--evidence-add`·`--revise`·`--hint-approve`)와 **읽는 눈**
   (`--resume-brief`). 2026-09-07 신설 · plan_26090715 §4.1·§4.4. `--hint-approve` 는 2026-09-21
   (plan_26092119 O6) — hint 발행 사전승인을 선언의 `hint_targets` 에 전사한다.
   왜: 거처(campaigns/)와 게이트(purge)는 계약대로 섰는데 **채우는 손이 대화 기억**이었다 —
   README 가 재개 에이전트에게 읽으라는 네 아티팩트의 저장소 내 producer 가 0 이었고,
   라이브에선 세션과 함께 소멸하는 스크래치패드 스크립트가 썼다(audit_26090708 §1.1).
   포맷 소유는 여기 하나이고 호출부는 각 phase 의 실제 실행 스크립트다(포맷 소유 1 · 호출부 N).

무결성 해시는 두지 않는다(`policy:GIT_SINGLE_AUTHORITY` · 사용자 결정 2026-09-06).
"""
from __future__ import annotations

import argparse
import json
import re
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


_VALIDATOR = None


def validator():
    """선언 규약(배정·모드·술어)의 **단일 소유자**는 검증기다. 여기서 파싱을 복제하면 두 자리가
    갈라지고, 갈라진 쪽이 조용히 늦는다(workflow.md §결정론 규율 — 개념 중복)."""
    global _VALIDATOR
    if _VALIDATOR is None:
        import importlib.util
        mod_path = Path(__file__).resolve().parent / "campaign_template_validator.py"
        if not mod_path.is_file():
            raise PurgeGateRefusal(f"검증기를 찾지 못했다: {_rel(mod_path)} — 배정 규약의 소유자가 없다")
        spec = importlib.util.spec_from_file_location("_campaign_validator", mod_path)
        if spec is None or spec.loader is None:
            raise PurgeGateRefusal("검증기 적재 실패")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _VALIDATOR = mod
    return _VALIDATOR


def read_declaration(base: Path) -> dict:
    doc = _read_json(base / "campaign.yaml")
    return doc if isinstance(doc, dict) else {}


def node_of_cell(base: Path, cell: str) -> str | None:
    """이 셀을 배정받은 노드. `--node` 를 안 줘도 상태가 노드를 갖게 하는 자리다 — 종전에는
    호출부가 플래그를 빠뜨리면 node_id 가 null 로 남았고, 그러면 증거가 노드를 잃는다."""
    doc = read_declaration(base)
    for node in validator().assignments_of(doc):
        if cell in validator().assigned_cells(doc, node):
            return node
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

    # 수명 상태(2026-10-17 · plan_26101719 D10·D14·D15). 보류는 "재개할 수 있다" 는 약속이므로 지우면 약속 위반이다 —
    # 종결 사유를 먼저 받는다. abandoned 는 축소 게이트로 이미 닫혔으니 그 게이트를 다시 묻는다(P1~P3 면제).
    life = fold_lifecycle(prev)
    if life["state"] in ("suspended", "resume_pending"):
        return [f"{previous_id}: 보류 상태({life['state']})는 purge 할 수 없다 — 사람이 포기를 고르면 "
                f"`--abandon {previous_id}` 로 종결 사유를 받은 뒤 지운다"]
    if life["state"] == "orphan":
        return [f"{previous_id}: 이름 없는 잔재(orphan · 수명 이벤트 없음 · ACTIVE 미지목)다 — 재개할 수 없고 "
                f"`--abandon {previous_id}`(축소 게이트)로만 정리한다"]
    if life["state"] == "unknown":
        return [f"{previous_id}: journey 에 이 버전이 모르는 수명 이벤트가 있다 — 브랜치 공통층 동기화 후 다시"]
    if life["state"] == "closed" and life.get("sub_convergence") == "unconverged":
        # K3(plan_26101722): purge 가 미수렴 기록을 지우면 서브 종결 누락이 다시 침묵 누락이 된다. 새 차단점이 아니라 기존
        # purge 게이트의 조건이다. 서브 미도달의 탈출구는 사람 수용 하나뿐이다.
        _miss = sorted(set(life["sub_nodes"]) - set(life["sub_echoed"]))
        _ctl = CLOSE_CONTROL.get(life["close_reason"], "close")
        return [f"{previous_id}: 서브 종결이 수렴하지 않았다({', '.join(_miss)}) — `relay.py --campaign-control {_ctl} "
                f"--campaign-id {previous_id} --node <sub>` 로 전하고 echo 를 받거나, 서브 미도달이면 사람이 "
                f"`--sub-unconverged-accept {previous_id} --node <sub> --approved-by <전사> --utc <T>` 로 수용한다"]
    if life["state"] == "closed" and life["close_reason"] == "abandoned":
        closed = next((e for e in reversed(read_journey(prev))
                       if isinstance(e, dict) and e.get("kind") == "campaign_closed"), {})
        return [f"{previous_id}: {r}" for r in abandoned_gate_reasons(previous_id, closed.get("summary_testlog"))]

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
    # ★ 2026-09-08(서브 라이브 지적): 종전에는 이 요구가 무조건이었다. 그런데 서브 인스턴스에는
    #   릴레이 원장이 **없다** — 왕복의 원장은 메인이 든다. 그래서 서브의 purge 게이트는 어떤
    #   조건에서도 열리지 않았고, 그것은 안전장치가 아니라 교착이다(헌법 노드제어 ③). 잃을 것이
    #   없으면 요약을 요구하지 않는다: 원장이 실재할 때만 그 서사를 묻는다.
    has_ledger = any(f.suffix == ".json" and f.name != "pending_hitl.json"
                     for f in (prev / "relay").iterdir()) if (prev / "relay").is_dir() else False
    if has_ledger and "relay_summary" not in kinds:
        reasons.append(f"{previous_id}: relay_summary 포인터가 없다 — 원장 원문은 휘발이므로 "
                       f"요약 testlog 가 없으면 왕복이 통째로 사라진다")
    # P1~P3 — 진행표가 실제로 완주를 표현하는가(2026-09-07 · plan_26090715 §4.4).
    # 종전 게이트는 증거 포인터만 물었다. 그래서 sweep 이 '돌았다'고 적은 셀의 cell.status 가
    # pending 인 채로, 선언된 노드의 phases/ 가 통째로 없는 채로 purge 가 열릴 수 있었다 —
    # 그 상태로 지우면 "돌지 않았다"와 "돌았는데 아무도 안 적었다"가 영원히 구분되지 않는다.
    reasons.extend(_instance_predicate_reasons(previous_id, prev))
    return reasons


def _instance_predicate_reasons(previous_id: str, prev: Path) -> list[str]:
    """검증기의 P1~P3 을 purge 선행조건으로 소비한다. 술어의 **단일 소유는 검증기**이고
    여기는 호출부다 — 두 자리에 적으면 갈라진다."""
    import importlib.util
    mod_path = Path(__file__).resolve().parent / "campaign_template_validator.py"
    if not mod_path.is_file():
        return [f"{previous_id}: 검증기(campaign_template_validator.py)를 찾지 못했다 — "
                f"P1~P3 을 물을 수단이 없다"]
    spec = importlib.util.spec_from_file_location("_campaign_validator", mod_path)
    if spec is None or spec.loader is None:
        return [f"{previous_id}: 검증기 적재 실패"]
    mod = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(mod)
        return [f"{previous_id}: {r}" for r in mod.instance_predicates(prev)]
    except Exception as exc:                       # noqa: BLE001 — 사유를 삼키지 않는다
        return [f"{previous_id}: P1~P3 판정 중 예외 — {exc!r}"]


def residue_write(out: dict, utc: str, camp_id: str | None = None) -> Path | None:
    """스캔 결과를 활성 인스턴스의 `residue.json` 에 남긴다(2026-09-08 · plan_26090813 §4.1).

    왜: 뼈대에 `residue.json` 이 있는데 그것을 **쓰는 손이 없었다** — 죽은 파일이었다. 틀만 있고
    산출이 없으면 다음 사람이 "스캔을 안 돌렸다"와 "돌렸는데 아무도 안 적었다"를 구분하지 못한다.
    `_bootstrap` 이면 쓰지 않는다(캠페인 밖은 상태를 갖지 않는다).
    """
    camp = camp_id or active_campaign_id()
    if camp == BOOTSTRAP:
        return None
    base = campaigns_dir() / camp
    if not base.is_dir():
        return None
    path = base / "residue.json"
    doc = {k: v for k, v in (_read_json(path) or {}).items() if k.startswith("_")}
    doc.update({"schema_version": 1, "scanned_utc": utc, **out})
    _write_json(path, doc)
    return path


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


def _sub_close_hint(camp_id: str, subs: list, control: str) -> str:
    """메인 종결의 서브 전파 안내(K2). 실행자는 relay 다 — 여기서는 명령만 말한다(메인은 서브를 직접 고치지 않는다)."""
    if not subs:
        return ""
    return (f" · 서브 종결은 A2A 제어명령으로 전한다: relay.py --campaign-control {control} --campaign-id {camp_id} "
            f"--node {'|'.join(subs)} … (echo 가 오기 전까지 '서브 미수렴' · purge 게이트가 이것을 묻는다)")


def close_campaign(camp_id: str, *, utc: str | None, apply: bool) -> str:
    """활성 캠페인을 **종결**한다 — ACTIVE 만 `_bootstrap` 으로 되돌리고 인스턴스는 지우지 않는다.

    왜 있나(2026-09-15 · plan_26091407 §5 ⑧ 싱글 서브 회차): ACTIVE 를 내리는 정식 경로가
    `--init --purge-previous`(= 다음 캠페인 개설) 하나뿐이었다. 그래서 캠페인 사이 창이 존재하지
    않았고, 종결된 multi 캠페인이 ACTIVE 로 남은 동안 single-node 체크아웃의 4자일치는 늘
    CAMPAIGN_MISMATCH 였다 — 반대 토폴로지 작업(싱글 서브 전파 등)을 여는 길이 ACTIVE 손편집이라는
    우회뿐이었다(D3: 우회 대신 경로를 고친다).

    선행조건은 purge 게이트와 **같다**(증거 포인터 전수 실재 · 릴레이 요약 · P1~P3). 종결 뒤에 증거가
    흩어지면 다음 init 의 purge 가 막히므로, 닫는 시점에 그 조건을 먼저 묻는다. 인스턴스는 남아
    다음 `--init --purge-previous <id>` 가 지운다. 멱등: 이미 `_bootstrap` 이면 no-op 이다
    (성공이 자기 검사를 깨뜨리지 않게).
    """
    if camp_id in RESERVED_IDS:
        raise PurgeGateRefusal(f"예약 id 는 종결 대상이 아니다: {camp_id!r}")
    active = active_campaign_id()
    if active == BOOTSTRAP:
        return f"no-op — 활성 캠페인이 없다(ACTIVE=_bootstrap · {_active_pointer_fact()})"
    if active != camp_id:
        raise PurgeGateRefusal(f"활성 캠페인은 {active!r} 다 — {camp_id!r} 를 종결할 수 없다"
                               f"(이름이 곧 증거 연결이다)")
    if not utc:
        raise PurgeGateRefusal("--close 는 --utc 가 필요하다(시각은 주입만 받는다 — 벽시계 금지)")
    reasons = purge_gate_reasons(camp_id)
    if reasons:
        raise PurgeGateRefusal("종결 선행조건(= purge 게이트)이 닫혀 있다:\n  - " + "\n  - ".join(reasons))
    if apply:
        subs = _sub_nodes(CAMPAIGNS / camp_id)
        writer_append_journey(CAMPAIGNS / camp_id, {
            "utc": utc, "kind": "campaign_closed", "reason": "completed",
            "lifecycle_v": LIFECYCLE_V, "source": "campaign_init --close", "sub_nodes": subs,
            "note": "ACTIVE → _bootstrap (인스턴스 보존 · 다음 --init --purge-previous 가 지운다)"})
        ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
        tail = _sub_close_hint(camp_id, subs, "close")
        # 계기 (a) — 파생 자식의 완주는 보류 중인 부모의 재개 질의를 정확히 한 번 단다.
        parent = _parent_on_close(CAMPAIGNS / camp_id, reason="completed", utc=utc)
        if parent:
            return (f"closed {camp_id} → ACTIVE=_bootstrap (인스턴스 보존: campaigns/{camp_id}) · "
                    f"부모 {parent} 에 재개 미결(계기 a)" + tail)
        return f"closed {camp_id} → ACTIVE=_bootstrap (인스턴스 보존: campaigns/{camp_id})" + tail
    return f"closed {camp_id} → ACTIVE=_bootstrap (인스턴스 보존: campaigns/{camp_id})"


def sweep_bootstrap_relay(*, apply: bool) -> list[str]:
    """`_bootstrap/relay/` 의 옛 원장을 비운다 (2026-09-07 · 사용자 결정 · plan_26090715 §4.10).

    `_bootstrap` 은 **캠페인 밖 릴레이 대기실**이다. 증거 포인터를 갖지 않으므로 purge 게이트의
    대상이 아니고, 그래서 게이트가 영구히 닫힌 채 옛 원장이 계속 쌓였다(2026-09-07 실측 6스레드).
    방치하면 다음 캠페인의 정지판정·상관검증과 섞인다 — 새 캠페인 init 때 **함께** 비운다.

    ★ 침묵 삭제 금지: 무엇을 지웠는지 목록으로 돌려주고 호출부가 출력한다. 조용한 삭제는
      "기록이 원래 없었던 것" 과 구분되지 않는다.
    """
    relay = campaigns_dir() / BOOTSTRAP / "relay"
    if not relay.is_dir():
        return []
    removed: list[str] = []
    for child in sorted(relay.iterdir()):
        if child.name == ".gitkeep":
            continue
        removed.append(_rel(child))
        if apply:
            if child.is_dir():
                shutil.rmtree(child)
            else:
                child.unlink()
    return removed


_STDIN_SLICE = None


def _load_slice(source: str) -> dict:
    """Read one main-derived declaration from a file or the A2A task body stdin."""
    global _STDIN_SLICE
    if source == "-":
        # stdin 은 한 번만 읽힌다 — 개설 정문(계보 확인)과 scaffold 가 같은 선언을 보게 붙잡아 둔다.
        if _STDIN_SLICE is None:
            try:
                _STDIN_SLICE = json.loads(sys.stdin.read())
            except ValueError as exc:
                raise PurgeGateRefusal(f"stdin 파생 선언 JSON을 읽지 못했다: {exc}") from exc
        doc = _STDIN_SLICE
    else:
        doc = _read_json(Path(source))
    if not isinstance(doc, dict):
        raise PurgeGateRefusal(f"파생 선언을 읽지 못했다: {source}")
    return doc


def scaffold(camp_id: str, plan_ref: str, *, apply: bool,
             from_slice: str | None = None) -> list[str]:
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
        # 안내문(`_` 접두 최상위 키)은 **뼈대에만** 산다(2026-09-08 · plan_26090813 D10).
        # 인스턴스로 복사되면 그 문장이 인스턴스의 사실인 척하고, 스캐너가 잡으면 사람이 지운다.
        doc = {k: v for k, v in doc.items() if not k.startswith("_")}
        if from_slice:
            # `-` 는 relay가 task body에 실은 한 JSON 선언이다. 중간 파일을 만들지 않는다.
            slice_doc = _load_slice(from_slice)
            if slice_doc.get("id") != camp_id:
                raise PurgeGateRefusal(
                    f"파생 선언의 id({slice_doc.get('id')!r})와 개설 이름({camp_id!r})이 다르다 "
                    f"— 이름이 곧 증거 연결이다")
            doc = {k: v for k, v in slice_doc.items() if not k.startswith("_")}
        doc["id"] = camp_id
        doc["plan_ref"] = doc.get("plan_ref") if from_slice else plan_ref
        (dest / "campaign.yaml").write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8")
        ep_path = dest / "evidence_pointers.json"
        ep = json.loads(ep_path.read_text(encoding="utf-8"))
        ep = {k: v for k, v in ep.items() if not k.startswith("_")}
        ep["campaign_id"] = camp_id
        ep.setdefault("pointers", [])
        ep_path.write_text(json.dumps(ep, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        ACTIVE_POINTER.write_text(camp_id + "\n", encoding="utf-8")
    made.append(f"campaigns/{camp_id}")
    return made


# ─────────────────────────────────────────────────────────────────────────────
# 단일 writer — campaigns/<id>/ 에 바이트를 쓰는 유일한 자리 (2026-09-07 · plan_26090715 §4.1)
#
# 왜 하나인가: 포맷이 여러 자리에 적히면 갈라지고, 갈라지면 검증기가 무엇을 기준으로 물을지
# 모르게 된다(`relay.record_attempt`·`simlog_writer` 와 같은 모양). 호출부는 N 개다 —
# build/serve/bench/publish 각 phase 의 **실제 실행 스크립트 종료부**가 이 CLI 를 부른다.
#
# ★ ACTIVE 가 `_bootstrap` 이면 **no-op** 이다(rc 0 · stderr 한 줄). 캠페인 밖 평시 서빙이
#   빈 인스턴스에 상태를 쓰기 시작하면 `_bootstrap` 이 캠페인 흉내를 내게 된다.

PHASE_STATES = ("pending", "running", "done", "failed")
PHASE_NAMES = ("build", "serve", "bench", "publish")
# ★ 2026-09-08 라이브 교정: 이 목록과 `classify_cell.py` 의 어휘가 **갈라져 있었다**. 분류기가
#   `measurement_void` 를 내면 writer 가 그 값을 모른다며 거부했고, 그래서 셀 상태·여정·bench
#   phase·브리핑이 **통째로 안 적혔다** — 측정은 성립했는데 기록만 사라진 것이다(이 계획이 고치려던
#   결함과 정확히 같은 계열). 분류 어휘의 소유는 `classify_cell.py` 이고 여기는 그것을 **포함**해야
#   한다. `_selftest` 의 tripwire 가 다음 분기를 커밋 전에 잡는다.
CELL_OUTCOMES = ("pending", "measured", "serve_failed", "build_failed", "void",
                 "measurement_void", "not_measured")
# 남은 작업으로 세는 값(브리핑·resume-brief 가 공유한다). `not_measured` 는 아무것도 재지 않았다는
# 뜻이라 남은 일이고, `measurement_void` 는 **잰 것이 파괴됐다**는 관측이라 반증 축이다.
CELL_OUTCOMES_PENDING = ("pending", "not_measured")
CELL_OUTCOMES_REFUTED = ("serve_failed", "build_failed", "void", "measurement_void")
# A sub keeps the main's plan_ref even when that file is not deployed. Its own execution
# narrative is a cell-scoped plan pointer; adding one never rewrites the declaration/approval.
EVIDENCE_KINDS = ("certificate", "bench_report", "sweep_map", "plan", "testlog", "devlog", "simlog",
                  "relay_summary")
FILL = "<<FILL>>"
JOURNEY_NAME = "journey.jsonl"
# ★ 2026-09-08(서브 라이브 지적): 자체검사 픽스처가 루트 `README.md` 를 증거 경로로 썼다. 그 파일은
#   메인 워크트리에만 있고 **서브 렌더 트리에는 없다** — 그래서 배달 검증으로 돌린 `--selftest` 가
#   그 노드에서만 FAIL 했다. 픽스처가 실물보다 좁은 것이 아니라 **한쪽 트리에만 맞았던** 것이다.
#   이 스크립트가 사는 어느 트리에서든 실재하는 파일을 쓴다 — 자기 자신이 그 조건을 만족한다.
_PORTABLE_EVIDENCE = ".claude/skills/terraforming_node/scripts/campaign_template_validator.py"


class WriterRefusal(Exception):
    """writer 가 쓰기를 거부했다. 값을 고쳐서 다시 부른다(우회 ✗)."""


def _writer_target(camp_id: str | None) -> "tuple[Path, str] | None":
    """쓸 인스턴스 경로. `_bootstrap` 이면 None(= no-op 신호)."""
    camp = camp_id or active_campaign_id()
    if camp == BOOTSTRAP:
        return None
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise WriterRefusal(f"인스턴스가 없다: campaigns/{camp} — 먼저 --init 하라")
    return base, camp


def _no_fill(**kv) -> None:
    for k, v in kv.items():
        if isinstance(v, str) and FILL in v:
            raise WriterRefusal(f"{k} 에 {FILL} 이 남아 있다 — 모르는 값을 그럴듯하게 채우지 않는다")


def _write_json(path: Path, doc: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
                    encoding="utf-8")


def writer_set_phase(base: Path, *, node: str, phase: str, state: str, predicate: str | None,
                     ok: bool, source: str | None, cell_id: str | None,
                     started_utc: str | None, ended_utc: str | None,
                     authored_by: str | None = None) -> Path:
    if phase not in PHASE_NAMES:
        raise WriterRefusal(f"phase 는 {PHASE_NAMES} 중 하나여야 한다: {phase!r}")
    if state not in PHASE_STATES:
        raise WriterRefusal(f"state 는 {PHASE_STATES} 중 하나여야 한다: {state!r}")
    if ok and not source:
        # 이 한 줄이 이 파일의 존재 이유 중 하나다 — 출처 없는 ok 는 단언이 검증을 대체한 것이고,
        # 그러면 깨진 순간을 아무도 모른다(campaigns/README.md §채우기 규칙).
        raise WriterRefusal("proof.ok=true 에는 --proof-source 가 필수다 "
                            "(proof 는 선언이 아니라 관측이다)")
    _no_fill(node=node, cell_id=cell_id or "", predicate=predicate or "", source=source or "")
    path = base / "phases" / node / f"{phase}.status.json"
    doc = _read_json(path) if path.is_file() else None
    doc = doc if isinstance(doc, dict) else {"schema_version": 1}
    doc.update({"schema_version": 1, "node_id": node, "phase": phase, "state": state,
                "_state_enum": list(PHASE_STATES),
                "proof": {"predicate": predicate, "ok": bool(ok), "source": source}})
    if cell_id is not None:
        doc["cell_id"] = cell_id
    if started_utc:
        doc["started_utc"] = started_utc
        # ★ `started_utc` 는 셀마다 덮어써져 **마지막 셀**을 가리킨다. 착수 시각을 묻는 술어(P4)가
        #   그걸 읽으면 늦은 값을 보고 통과한다 — 처음 한 번만 적히는 자리를 따로 둔다.
        if not doc.get("first_started_utc"):
            doc["first_started_utc"] = started_utc
    if ended_utc:
        doc["ended_utc"] = ended_utc
    if authored_by:
        # 누가 적었는가. 서브 진행표가 서브 저작인지 메인의 사후 재저작인지는 이 필드로만 갈린다
        # (P5). 2026-09-07 에는 `phases/sub/*` 4개가 같은 초에 메인 손으로 나타났다.
        doc["authored_by"] = authored_by
    doc.setdefault("authored_by", None)
    doc.setdefault("first_started_utc", None)
    doc.setdefault("started_utc", None)
    doc.setdefault("ended_utc", None)
    doc.setdefault("cell_id", None)
    _write_json(path, doc)
    return path


def _sweep_measurement(sweep_path: Path, cell: str) -> "tuple[float | None, str]":
    """sweep 레코드에서 동시성 1 디코드 처리량을 읽는다. 값 아니면 **왜 없는지**를 돌려준다.

    왜 writer 가 직접 읽는가(2026-09-08 · plan_26090813 D19): 종전에는 호출부가
    `--measurement-decode-tps` 를 넘겨야 했는데 `broad_search.sh` 의 호출부가 그 플래그를 아예
    안 넘겼다. sweep 에는 19.12 t/s 가 적혀 있는데 cell.status 의 측정은 null 이었다 — 값이 이미
    있는 파일을, 쓰는 손이 직접 읽으면 그 결손이 생기지 않는다.
    """
    doc = _read_json(sweep_path)
    if not isinstance(doc, dict):
        return None, f"sweep 기록을 읽지 못했다: {_rel(sweep_path)}"
    for rec in (doc.get("cells") or []):
        if not isinstance(rec, dict) or rec.get("cell_key") != cell:
            continue
        vec = rec.get("concurrency_vector")
        val = vec.get("1") if isinstance(vec, dict) else None
        if isinstance(val, (int, float)) and not isinstance(val, bool):
            return float(val), (f"sweep:{_rel(sweep_path)}"
                                f"#cells[cell_key={cell}].concurrency_vector.1")
        return None, (f"sweep 레코드에 동시성 1 측정이 없다: {_rel(sweep_path)} cell_key={cell}")
    return None, f"sweep 기록에 셀 {cell!r} 이 없다: {_rel(sweep_path)}"


def _sweep_image(sweep_path: Path, cell: str) -> "tuple[dict | None, str]":
    """sweep 레코드에서 **이미지 정체성**(digest·tag)을 읽는다 (2026-09-11 · plan_26091108 R5).

    왜 셀 좌표에 이미지 축이 필요한가: 셀 간 비교는 **같은 이미지 안에서만** 성립한다. 태그는
    이름이라 같은 태그가 다른 내용을 가리킬 수 있고(2026-08 실측: 0.27.1 소스 빌드가 wheel 태그로
    나갔다), 그때 지도는 두 다른 빌드를 한 축 위에 나란히 놓는다. digest 는 내용이다.
    측정 평면에는 이미 있었고(sweep meta) **셀 평면으로 올라오지 않았다** — 그 한 홉이 빠져서
    캠페인 아티팩트만 보면 어떤 이미지로 잰 것인지 알 수 없었다.
    """
    doc = _read_json(sweep_path)
    if not isinstance(doc, dict):
        return None, f"sweep 기록을 읽지 못했다: {_rel(sweep_path)}"
    for rec in (doc.get("cells") or []):
        if not isinstance(rec, dict) or rec.get("cell_key") != cell:
            continue
        coord = rec.get("coordinates") if isinstance(rec.get("coordinates"), dict) else {}
        dig, tag = coord.get("image_digest"), coord.get("image_tag")
        if not dig:
            return None, (f"sweep 레코드에 image_digest 가 없다: {_rel(sweep_path)} "
                          f"cell_key={cell} (측정 평면이 그 좌표를 싣지 않았다)")
        return ({"digest": str(dig), "tag": (str(tag) if tag else None),
                 "source": (f"sweep:{_rel(sweep_path)}"
                            f"#cells[cell_key={cell}].coordinates.image_digest")}, "")
    return None, f"sweep 기록에 셀 {cell!r} 이 없다: {_rel(sweep_path)}"


def _sweep_output(base: Path, sweep_path: Path, cell: str, node: str | None) -> dict | None:
    """Carry the producer's measured output path; never guess a new run from a directory name."""
    state = _read_json(sweep_path)
    records = state.get("cells") if isinstance(state, dict) else None
    hits = [r for r in (records or []) if isinstance(r, dict) and r.get("cell_key") == cell]
    if not hits or all("sweep_dir" not in r for r in hits):
        return None                         # legacy producer; fixed-path consumers remain compatible
    if len(hits) != 1:
        raise WriterRefusal(f"sweep 출력 경로의 셀 기록이 유일하지 않다: {cell}")
    rec = hits[0]
    rel, stamp = rec.get("sweep_dir"), rec.get("sweep_generated_utc")
    if rel is None and stamp is None:
        return None                         # no measured output was carried
    if rec.get("cell_outcome") != "measured" or not isinstance(rel, str) or not isinstance(stamp, str) or not stamp:
        raise WriterRefusal("sweep 출력 경로는 measured 셀의 실제 경로·generated_utc 짝이어야 한다")
    decl = read_declaration(base)
    topology = next((n.get("topology") for n in (decl.get("nodes") or [])
                     if isinstance(n, dict) and n.get("node_id") == node), None) or _declared_topology(base)
    parts = Path(rel).parts
    if topology not in ("single", "multi") or Path(rel).is_absolute() or ".." in parts \
            or len(parts) != 4 or parts[:3] != ("output", topology, "benchlog") \
            or not re.fullmatch(r"sweep_[A-Za-z0-9][A-Za-z0-9._-]*", parts[-1]):
        raise WriterRefusal(f"sweep 출력 경로는 이 topology의 benchlog 아래여야 한다: {rel!r}")
    path = REPO_ROOT / rel
    root = REPO_ROOT / "output" / topology / "benchlog"
    if any(p.is_symlink() for p in (path, *path.parents) if p != REPO_ROOT and REPO_ROOT in p.parents):
        raise WriterRefusal(f"sweep 출력 경로에 symlink가 있다: {rel}")
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError as exc:
        raise WriterRefusal(f"sweep 출력 경로가 benchlog 밖이다: {rel}") from exc
    index_path = path / "sweep_index.json"
    if index_path.is_symlink() or not index_path.is_file():
        raise WriterRefusal("sweep 출력 index는 실재 정규파일이어야 한다(symlink 불가)")
    index = _read_json(index_path)
    meta = index.get("meta") if isinstance(index, dict) and isinstance(index.get("meta"), dict) else {}
    config = rec.get("config")
    if not isinstance(index, dict) or not isinstance(config, str) or not config \
            or (meta.get("config_name") or index.get("config")) != config \
            or index.get("generated_utc") != stamp \
            or meta.get("topology") not in (None, topology):
        raise WriterRefusal("sweep 출력 index의 config/UTC가 공식 셀 기록과 일치하지 않는다")
    return {"sweep_dir": rel, "sweep_generated_utc": stamp, "sweep_config": config,
            "sweep_source": f"sweep:{_rel(sweep_path)}#cells[cell_key={cell}].sweep_dir"}


# ─────────────────────────────────────────────────────────────────────────────
# 셀 출처 (2026-09-14 · plan_26091407 §4.0 · 사용자 결정 Q1·Q8)
#
# 두 자리가 있다. **진입 precheck**(`lockset_precheck` ← `broad_search.sh cell`)는 표시 부재·무효만
# fail-closed 로 막고, **불일치 기재**(`_cell_provenance` ← `--cell-set`)는 선언과 서빙 실물의 드리프트를
# cell.status 에 적을 뿐 막지 않는다. 표시 어휘의 소유는 검증기(`LOCKSET_PROVENANCE`)이고 여기는 호출부다.

# 대조 수단이 아직 없는 노브의 이름표. 이름으로 남기는 이유: 대조하지 않은 노브를 "불일치 0건"으로 읽으면
# 안 쟀다와 일치했다가 구분되지 않는다. 2026-09-14 단계 ②(plan_26091407 §4.2·§4.3)가 explorer 파생값을 lockset 에
# 기계 각인하게 되면서 batch·KV 클램프도 대조 수단을 얻었다(아래 _LOCKSET_KNOB_COMPARE) — 지금은 비어 있다.
PROVENANCE_PENDING_KNOBS: tuple[str, ...] = ()

# explorer 파생 노브의 대조: lockset(explorer 가 잠근 값) ↔ sweep 기록(서빙 yaml 이 실제로 실은 값).
#   (lockset 키, sweep 블록, sweep 키, lockset *_source 키). sweep 키 이름은 broad_search.sh 가 셀 기록에 싣는
#   이름 그대로다(coordinates.max_num_seqs = 서빙 yaml max-num-seqs · capacity.kv_cache_memory_bytes = 클램프).
_LOCKSET_KNOB_COMPARE = (
    ("batch", "coordinates", "max_num_seqs", "batch_source"),
    ("kv_cache_memory_bytes", "capacity", "kv_cache_memory_bytes", "kv_source"),
)


def _active_pointer_fact() -> str:
    """ACTIVE 포인터의 **실제 상태** 한 줄. `_bootstrap` 으로 접힌 이유를 판정 사유에 싣는다 —
    포인터 부재·무효 이름(오타·purge 뒤 잔존)·명시 `_bootstrap` 이 같은 문장으로 보이면 운영자는
    "캠페인 밖 호출" 과 "포인터를 잃은 캠페인 호출" 을 구분하지 못한다."""
    if not ACTIVE_POINTER.is_file():
        return "ACTIVE 포인터 부재"
    try:
        name = ACTIVE_POINTER.read_text(encoding="utf-8").strip()
    except OSError as exc:
        return f"ACTIVE 포인터 읽기 실패({exc})"
    if not name:
        return "ACTIVE 포인터가 비었다"
    if name == BOOTSTRAP:
        return "ACTIVE=_bootstrap(명시)"
    if not (CAMPAIGNS / name).is_dir():
        return f"ACTIVE={name!r} 무효(campaigns/{name} 디렉터리 없음)"
    return f"ACTIVE={name}"


def lockset_precheck(cell: str, camp_id: str | None = None) -> dict:
    """측정 진입 전 셀 출처 precheck. `status` ∈ passed | refused | not_applicable.

    ★ `_bootstrap`(활성 캠페인 없음)은 **not_applicable** 이다. 캠페인 밖 호출에는 셀 입력의 거처
      자체가 없고(writer 도 no-op 인 같은 경계), 거기서 막으면 해소 경로가 "대기실에 lockset 을
      만든다" 뿐이라 `_bootstrap` 이 캠페인 흉내를 내게 된다(D3: 게이트가 우회를 만든다). 대신
      조용히 넘기지 않는다 — 호출부가 이 판정을 stderr 와 셀 기록에 **이름으로** 남긴다.
    """
    camp = camp_id or active_campaign_id()
    if camp == BOOTSTRAP:
        return {"status": "not_applicable", "campaign_id": BOOTSTRAP, "cell": cell,
                "lockset": None, "provenance": None,
                "reason": (f"{_active_pointer_fact() if camp_id is None else '--campaign-id=_bootstrap'}"
                           f" → _bootstrap — 캠페인 셀 문맥이 없어 lockset 의 거처가 없다"
                           f"(writer no-op 과 같은 경계). 셀 출처 precheck 대상이 아니다")}
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise WriterRefusal(f"인스턴스가 없다: campaigns/{camp} — 출처를 물을 셀이 없다")
    path = base / "cells" / cell / "lockset.json"
    why = validator().lockset_provenance_reason(path)
    doc = _read_json(path) if path.is_file() else None
    prov = doc.get("provenance") if isinstance(doc, dict) else None
    if why is not None and node_of_cell(base, cell) is None:
        # R10 계열: 셀 키 어휘가 캠페인 셀 id 와 갈라지면 lockset 이 "없는" 것으로 보인다.
        why += (f" · 셀 키 {cell!r} 가 campaigns/{camp}/campaign.yaml 의 assignments 에도 없다"
                f"(스윕 셀 키가 캠페인 셀 id 와 같은 이름인지 확인하라)")
    return {"status": "refused" if why else "passed", "campaign_id": camp, "cell": cell,
            "lockset": _rel(path), "provenance": prov if isinstance(prov, str) else None,
            "reason": why}


def _sweep_coordinate(sweep_path: Path, cell: str, key: str,
                      block: str = "coordinates") -> "tuple[object, str]":
    """sweep 레코드 좌표 하나(`block` = coordinates | capacity). (값, 출처) — 값이 없으면 (None, 없는 이유)."""
    doc = _read_json(sweep_path)
    if not isinstance(doc, dict):
        return None, f"sweep 기록을 읽지 못했다: {_rel(sweep_path)}"
    for rec in (doc.get("cells") or []):
        if not isinstance(rec, dict) or rec.get("cell_key") != cell:
            continue
        coord = rec.get(block) if isinstance(rec.get(block), dict) else {}
        if key not in coord or coord.get(key) in (None, "", "NA"):
            return None, (f"sweep 레코드에 {block}.{key} 가 없다: {_rel(sweep_path)} cell_key={cell} "
                          f"(서빙 yaml 에 그 키가 없었거나 측정 평면이 싣지 않았다)")
        return coord.get(key), (f"sweep:{_rel(sweep_path)}#cells[cell_key={cell}].{block}.{key}")
    return None, f"sweep 기록에 셀 {cell!r} 이 없다: {_rel(sweep_path)}"


def _as_number(val) -> "float | None":
    if isinstance(val, bool):
        return None
    try:
        return float(val)
    except (TypeError, ValueError):
        return None


def _declared_target_gmu(cfg_path: Path) -> "tuple[float | None, str]":
    """셀 config 의 선언 target_gpu.target_gmu. (값, 출처) — 없으면 (None, 없는 이유).

    config.yaml 은 YAML 이라 PyYAML 을 **지연 import** 한다. 없으면 대조 불가를 사유로 돌려준다 —
    이 자리는 기재이지 게이트가 아니므로 fail-loud 기재(gap)가 정당하고, 조용히 '일치'로 접지 않는다.
    """
    if not cfg_path.is_file():
        return None, f"셀 config 부재: {_rel(cfg_path)}"
    try:
        import yaml
    except ImportError:
        return None, "PyYAML 부재 — 선언 target_gmu 를 읽지 못했다(대조 불가 · 기재)"
    try:
        doc = yaml.safe_load(cfg_path.read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as exc:
        return None, f"셀 config 파손({_rel(cfg_path)}): {exc}"
    tgt = doc.get("target_gpu") if isinstance(doc, dict) else None
    val = _as_number(tgt.get("target_gmu")) if isinstance(tgt, dict) else None
    if val is None:
        return None, f"셀 config 에 target_gpu.target_gmu 가 수로 선언되지 않았다: {_rel(cfg_path)}"
    return val, f"{_rel(cfg_path)}#target_gpu.target_gmu"


def _lockset_stamp(ldoc: dict) -> "str | None":
    """lockset 표시가 누구 손에서 왔는가. explorer-phase2 ∧ trial_provenance(어휘 안) = `machine` · explorer-phase2 만 =
    `self-declared` · hand-authored = `hand` · 그 외(부재·어휘 밖) = None. 판정이 아니라 옮겨 적기다(기재)."""
    prov = ldoc.get("provenance")
    if prov == "hand-authored":
        return "hand"
    if prov != "explorer-phase2":
        return None
    return "machine" if ldoc.get("trial_provenance") in ("measured", "mock", "dry-run") else "self-declared"


def _cell_provenance(base: Path, cell: str, node: str | None, sweep_state: str | None,
                     prior: dict, prior_mismatch: list) -> "tuple[dict, list]":
    """셀 출처 표시 + 선언↔서빙 실물 드리프트. 반환 `(provenance, provenance_mismatch)`.

    ★ 관측 불가와 부재를 가른다: 메인 인스턴스에서 서브 배정 셀(회수 편입·사후 수리 경로)은 lockset 을
      서브 인스턴스가 저작하고 메인은 읽지 않는다. 그 셀에 "lockset 부재 … hand-authored 로 표시" 를
      적으면 관측 불가가 부재로 기재되고 메인에게 서브 셀에 거짓 출처를 붙이라는 처방까지 준다.
      판정은 P6 와 같은 함수(`lockset_observable`)가 한다.
    ★ 대조하지 못한 노브는 **직전 기재를 유지**한다 — 부재(sweep 없음·좌표 NA·PyYAML 부재)가 과거
      관측을 덮으면 안 된다. 이번 호출이 실제로 대조한 노브(`compared`)의 기재만 교체된다.
    """
    decl = read_declaration(base)
    pending = list(PROVENANCE_PENDING_KNOBS)
    if not validator().lockset_observable(decl, node):
        return ({"observability": "not_observable",
                 "observability_reason": (f"서브 배정 셀(node={node}) — lockset 은 서브 인스턴스가 저작하고 "
                                          f"precheck 는 측정 스킬이 tool_plane 으로 열린 서브의 진입에서 집행한다"
                                          f"(node_role_contract — tool_plane 이 비는 서브에는 이 precheck 의 집행자가 없고, "
                                          f"서브 배정은 토폴로지 오케스트레이션 전략이 정한다). "
                                          f"메인은 서브 인스턴스를 읽지 않으며 브리핑은 lockset 출처를 싣지 않는다(헌법 노드제어 ①)"),
                 "lockset": None, "lockset_source": None, "lockset_gap": None,
                 "compared": [], "compare_gaps": [], "pending_knobs": pending},
                list(prior_mismatch))
    lock = base / "cells" / cell / "lockset.json"
    ldoc = _read_json(lock) if lock.is_file() else None
    ldoc = ldoc if isinstance(ldoc, dict) else {}
    prov = {"observability": "observed", "observability_reason": None,
            "lockset": ldoc.get("provenance") if isinstance(ldoc.get("provenance"), str) else None,
            "lockset_source": _rel(lock) if lock.is_file() else None,
            # 출처 표시가 성립하지 않는 사유(precheck 와 같은 함수). 진입에서 이미 막혔을 셀이라도
            # 여기서는 사실로만 적는다 — 기재 자리가 게이트를 흉내 내지 않는다.
            "lockset_gap": validator().lockset_provenance_reason(lock),
            # 기계 각인 여부(2026-09-14 · plan_26091407 단계 ② · 리뷰 교정): explorer 수렴 각인은 trial_provenance
            # (measured|mock|dry-run)를 함께 적는다. 그 칸 없이 explorer-phase2 만 있으면 절차 자기선언이다 — 둘을
            # 같은 표시로 옮기면 소비자가 mock 수렴·손 라벨을 실측 각인과 가르지 못한다(기재 · 차단 ✗).
            "lockset_trial_provenance": (ldoc.get("trial_provenance")
                                         if isinstance(ldoc.get("trial_provenance"), str) else None),
            "lockset_stamp": _lockset_stamp(ldoc),
            "compared": list(prior.get("compared") or []),
            "compare_gaps": list(prior.get("compare_gaps") or []),
            "pending_knobs": pending}
    if not sweep_state:
        return prov, list(prior_mismatch)
    fresh: list[dict] = []
    compared: list[str] = []
    gaps: list[str] = []
    declared, dsrc = _declared_target_gmu(base / "cells" / cell / "config.yaml")
    observed_raw, osrc = _sweep_coordinate(Path(sweep_state), cell, "gpu_memory_utilization")
    observed = _as_number(observed_raw)
    if declared is None:
        gaps.append(f"gpu_memory_utilization 대조 불가 — {dsrc} (직전 기재 유지)")
    elif observed is None:
        gaps.append("gpu_memory_utilization 대조 불가 — "
                    + (osrc if observed_raw is None
                       else f"서빙 gmu 가 수가 아니다: {observed_raw!r} ({osrc})")
                    + " (직전 기재 유지)")
    else:
        compared.append("gpu_memory_utilization")
        if declared != observed:
            fresh.append({"knob": "gpu_memory_utilization",
                          "declared": declared, "declared_source": dsrc,
                          "observed": observed, "observed_source": osrc,
                          "lockset_provenance": prov["lockset"],
                          "lockset_gmu_source": ldoc.get("gmu_source")})
    # explorer 파생 노브(2026-09-14 · plan_26091407 §4.2 · 단계 ① 이 남긴 슬롯): lockset 에 잠긴 값 ↔ 서빙 실물.
    #   lockset 쪽이 비었으면(파일 부재·값 null = "아직 정하지 않았다") 대조 불가로 적는다 — null 을 "없음과 일치"
    #   로 읽으면 explorer 가 정하지 않은 노브에 사람이 손으로 넣은 서빙 값이 조용히 정상으로 보인다.
    #   ★ lockset 이 비었는데(null = "아직 정하지 않았다") 서빙 yaml 이 수를 싣고 있으면 **불일치로 기재**한다
    #     (declared=null · declared_state=undetermined) — F1 형태(lockset 은 침묵, yaml 에 손값 8)가 바로 이것이다.
    #     양쪽 다 비었을 때만 대조 불가다(리뷰 교정 2026-09-14: 종전 초안은 lockset null 을 sweep 을 읽기 전에 대조
    #     불가로 접어 손값이 provenance_mismatch[] 에 영영 닿지 않았다).
    for lkey, block, skey, src_key in _LOCKSET_KNOB_COMPARE:
        declared_k = _as_number(ldoc.get(lkey))
        observed_raw_k, osrc_k = _sweep_coordinate(Path(sweep_state), cell, skey, block)
        observed_k = _as_number(observed_raw_k)
        if observed_k is None:
            gaps.append(f"{lkey} 대조 불가 — "
                        + (osrc_k if observed_raw_k is None
                           else f"서빙 값이 수가 아니다: {observed_raw_k!r} ({osrc_k})")
                        + ("" if declared_k is not None
                           else f" · lockset {lkey} 도 수로 잠겨 있지 않다({prov['lockset_source'] or 'lockset 부재'})")
                        + " (직전 기재 유지)")
            continue
        compared.append(lkey)
        if declared_k is None:
            fresh.append({"knob": lkey,
                          "declared": None, "declared_state": "undetermined",
                          "declared_source": (f"{prov['lockset_source']}#{lkey}" if prov["lockset_source"]
                                              else "lockset 부재"),
                          "observed": observed_k, "observed_source": osrc_k,
                          "lockset_provenance": prov["lockset"],
                          f"lockset_{src_key}": ldoc.get(src_key)})
        elif declared_k != observed_k:
            fresh.append({"knob": lkey,
                          "declared": declared_k, "declared_source": f"{prov['lockset_source']}#{lkey}",
                          "observed": observed_k, "observed_source": osrc_k,
                          "lockset_provenance": prov["lockset"],
                          f"lockset_{src_key}": ldoc.get(src_key)})
    # 손레버 batch ↔ explorer 재계산(plan §4.0 Q1 "hand 면 재계산과 대조해 불일치 기재"): explorer 는 손레버를 덮어쓰지
    #   않고 같은 산식이 냈을 값을 lockset `batch_derivation.derived_batch_would_be` 에 남긴다. yaml↔lockset 대조는
    #   손레버 셀에서 자명하게 일치하므로(둘 다 손값) 이 대조가 없으면 "손 8 vs KV-fit 64" 가 cell.status 어디에도 없다.
    if ldoc.get("batch_source") == "hand-lever":
        der = ldoc.get("batch_derivation") if isinstance(ldoc.get("batch_derivation"), dict) else {}
        hand_b, would_b = _as_number(ldoc.get("batch")), _as_number(der.get("derived_batch_would_be"))
        if hand_b is None or would_b is None:
            gaps.append("batch_hand_lever 대조 불가 — lockset 손레버 batch 또는 explorer 재계산값"
                        "(batch_derivation.derived_batch_would_be)이 수가 아니다 (직전 기재 유지)")
        else:
            compared.append("batch_hand_lever")
            if hand_b != would_b:
                fresh.append({"knob": "batch_hand_lever",
                              "declared": hand_b, "declared_source": f"{prov['lockset_source']}#batch(batch_source=hand-lever)",
                              "observed": would_b,
                              "observed_source": f"{prov['lockset_source']}#batch_derivation.derived_batch_would_be",
                              "observed_source_kind": der.get("derived_batch_source_would_be"),
                              "lockset_provenance": prov["lockset"]})
    prov["compared"] = compared
    prov["compare_gaps"] = gaps
    kept = [m for m in prior_mismatch if isinstance(m, dict) and m.get("knob") not in compared]
    return prov, kept + fresh


# ── 셀 입력 materialize (2026-10-16 · plan_26101622 F1) ─────────────────────────────────────
#   셀 디렉터리(config.yaml · lockset.json · cell.status.json)를 뼈대에서 만드는 쓰기 문이 없었다 — 메인은 사람(에이전트)이
#   손으로 채웠고 서브는 그 단계를 몰라 hint 2건이 셀 입력 결손 3건을 안고 발행됐다(plan_26101622 Q2). 값은 두 종류다:
#   **파생**(서빙 yaml · env · manifest · /proc/meminfo 에서 읽힌다 — 손으로 적으면 두 자리가 갈라진다)과 **선언**
#   (사람이 정하는 축 — `--set 키=값` 으로만 받는다 · 요구가 없으면 `키=null` 을 명시). 빈칸이 하나라도 남으면 쓰지 않는다.
MATERIALIZE_SET_KEYS = ("vram_budget_gb", "per_card_vram_gib", "ple_mode", "control_variable",
                        "concurrency_requirement", "typical_request_tokens", "tiktoken_host_path",
                        "nas_host_root", "target_gmu", "variant")   # 닫힌 목록 — 파생 키도 명시 --set 이 이긴다(출처 주석에 남는다)


def _yaml_scalar(v) -> str:
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(v)
    t = str(v)
    return t if re.fullmatch(r"[A-Za-z0-9_./+-]+", t) and t.lower() not in ("null", "true", "false", "yes", "no") \
        else json.dumps(t, ensure_ascii=False)


def _env_value(path: Path, key: str) -> "str | None":
    if not path.is_file():
        return None
    for ln in path.read_text(encoding="utf-8").splitlines():
        m = re.match(rf"^\s*{re.escape(key)}=(.*)$", ln)
        if m:
            return m.group(1).strip().strip('"').strip("'")
    return None


def _meminfo_gib(path: Path = Path("/proc/meminfo")) -> "float | None":
    try:
        m = re.search(r"^MemTotal:\s+(\d+)\s+kB", path.read_text(encoding="utf-8"), re.M)
    except OSError:
        return None
    return round(int(m.group(1)) / (1024 * 1024), 1) if m else None


def writer_materialize_cell(base: Path, *, cell: str, repo: Path, sets: "dict[str, str]", utc: str,
                            meminfo: Path = Path("/proc/meminfo")) -> "list[Path]":
    """배정된 셀의 셀 입력 3파일을 뼈대에서 만든다. 덮어쓰지 않는다(있으면 거부 — 재작성은 사람이 지우고 다시)."""
    import yaml
    if not utc:
        raise WriterRefusal("--cell-materialize 는 --utc 가 필요하다(시각은 주입만)")
    node = node_of_cell(base, cell)
    if node is None:
        raise WriterRefusal(f"셀 {cell!r} 이 assignments 에 없다 — 배정되지 않은 셀은 만들지 않는다(이름이 곧 증거 연결)")
    bad = sorted(set(sets) - set(MATERIALIZE_SET_KEYS))
    if bad:
        raise WriterRefusal(f"--set 닫힌 목록 밖 키 {bad} — 허용 {list(MATERIALIZE_SET_KEYS)}")
    cdir = base / "cells" / cell
    names = ("config.yaml", "lockset.json", "cell.status.json")
    have = [n for n in names if (cdir / n).exists()]
    if have:
        raise WriterRefusal(f"cells/{cell}/ 에 이미 {have} 가 있다 — 덮어쓰지 않는다")
    decl = read_declaration(base) or {}
    nd = next((n for n in decl.get("nodes") or [] if isinstance(n, dict) and n.get("node_id") == node), {})
    topo = nd.get("topology") or (decl.get("control_variables") or {}).get("topology")
    if topo not in ("single", "multi"):
        raise WriterRefusal(f"노드 {node!r} 의 topology 를 선언에서 해소하지 못했다: {topo!r}(추측 ✗)")
    syaml = repo / "output" / topo / "configs" / f"{cell}.yaml"
    if not syaml.is_file():
        raise WriterRefusal(f"서빙 yaml {syaml.relative_to(repo)} 부재 — 셀 id = config 이름이어야 한다")
    sv = yaml.safe_load(syaml.read_text(encoding="utf-8")) or {}
    env = repo / "output" / topo / "envs" / f".env.{cell}"
    mf_path = repo / "output" / topo / "manifest.yaml"
    mf = yaml.safe_load(mf_path.read_text(encoding="utf-8")) if mf_path.is_file() else {}
    mf = mf if isinstance(mf, dict) else {}
    derived = {
        "path": (sv.get("model"), f"output/{topo}/configs/{cell}.yaml#model"),
        "context_len": (sv.get("max-model-len"), f"서빙 yaml#max-model-len"),
        "kv_cache_dtype": (sv.get("kv-cache-dtype", "auto"), "서빙 yaml#kv-cache-dtype(부재 = vLLM 기본 auto)"),
        "tp": (sv.get("tensor-parallel-size", 1), "서빙 yaml#tensor-parallel-size(부재 = 1)"),
        "target_gmu": (sv.get("gpu-memory-utilization"), "서빙 yaml#gpu-memory-utilization(서빙된 값)"),
        "variant": (_env_value(env, "VARIANT") or "stock", f".env.{cell}#VARIANT(부재 = stock)"),
        "served_model_name": (_env_value(env, "SERVING_MODEL_NAME") or sv.get("served-model-name"),
                              f".env.{cell}#SERVING_MODEL_NAME"),
        "config_name": (cell, "셀 id"),
        "gpu_model": (mf.get("gpu_model"), f"output/{topo}/manifest.yaml#gpu_model"),
        "nas_host_root": (mf.get("nas_model_path") if mf.get("model_source") == "managed" else None,
                          f"output/{topo}/manifest.yaml#nas_model_path(model_source=managed)"),
        "test_device_total_gib": (_meminfo_gib(meminfo), "measured /proc/meminfo MemTotal"),
    }
    vals: dict = {}
    src: dict = {}
    for k, (v, why) in derived.items():
        if v is not None:
            vals[k], src[k] = v, why
    for k, raw in sets.items():
        vals[k], src[k] = yaml.safe_load(raw) if raw.strip() else None, "--set(사람 선언)"
    text = (TEMPLATE / "cells" / "_cell" / "config.yaml").read_text(encoding="utf-8")
    left: list[str] = []

    def _sub(m):
        k = m.group(2)
        if k in vals:
            return f"{m.group(1)}{_yaml_scalar(vals[k])}"
        left.append(k)
        return m.group(0)
    text = re.sub(r"^(\s*([a-z_]+):\s*)<<FILL>>", _sub, text, flags=re.M)
    if left:
        raise WriterRefusal(f"채우지 못한 빈칸 {left} — 파생할 수 없는 선언 축이다. `--set 키=값`(요구 없음 = `키=null`)으로 적는다")
    head = (f"# materialized: campaign_init --cell-materialize {cell} · {utc} · node {node}\n"
            + "".join(f"#   {k}: {src[k]}\n" for k in sorted(src)))
    lock = _read_json(TEMPLATE / "cells" / "_cell" / "lockset.json")
    lock.update({"id": cell, "provenance": "hand-authored", "max_model_len": vals.get("context_len"),
                 "quantization": sv.get("quantization")})
    st = _read_json(TEMPLATE / "cells" / "_cell" / "cell.status.json")
    st.update({"cell_id": cell, "node_id": node})
    cdir.mkdir(parents=True, exist_ok=True)
    (cdir / "config.yaml").write_text(head + text, encoding="utf-8")
    _write_json(cdir / "lockset.json", lock)
    _write_json(cdir / "cell.status.json", st)
    return [cdir / n for n in names]


def writer_set_cell(base: Path, *, cell: str, outcome: str, node: str | None,
                    version: str | None, model: str | None,
                    decode_tps: str | None, measurement_source: str | None,
                    void_reason: str | None, void_reason_source: str | None,
                    axis_citation: str | None, next_intent: str | None,
                    utc: str | None, sweep_state: str | None = None,
                    image_digest: str | None = None, image_tag: str | None = None,
                    lite_raw: str | None = None
                    ) -> "tuple[Path, Path | None]":
    if outcome not in CELL_OUTCOMES:
        raise WriterRefusal(f"cell_outcome 은 {CELL_OUTCOMES} 중 하나여야 한다: {outcome!r}")
    if void_reason and not void_reason_source:
        raise WriterRefusal("--void-reason 에는 --void-reason-source 가 필수다(출처 없는 판정 ✗)")
    if node is None:
        # 배정에서 파생한다 — 호출부가 플래그를 빠뜨리면 node_id 가 null 로 남고, null 인 상태는
        # 증거가 노드를 잃은 상태다(P2 공허 통과의 뿌리).
        node = node_of_cell(base, cell)
    _no_fill(cell=cell, node=node or "", version=version or "", model=model or "")
    gap: str | None = None
    if decode_tps is None and sweep_state:
        val, why = _sweep_measurement(Path(sweep_state), cell)
        if val is None:
            gap = why
        else:
            decode_tps, measurement_source = str(val), why
    sweep_output = _sweep_output(base, Path(sweep_state), cell, node) if sweep_state else None
    if sweep_output and outcome != "measured":
        raise WriterRefusal("measured sweep 출력 경로와 셀 outcome이 모순이다")
    path = base / "cells" / cell / "cell.status.json"
    doc = _read_json(path) if path.is_file() else None
    doc = doc if isinstance(doc, dict) else {"schema_version": 1}
    doc.update({"schema_version": 1, "cell_id": cell, "cell_outcome": outcome,
                "_cell_outcome_enum": list(CELL_OUTCOMES)})
    for key, val in (("node_id", node), ("version", version), ("model", model)):
        if val is not None:
            doc[key] = val
        doc.setdefault(key, None)
    meas = doc.get("measurement") if isinstance(doc.get("measurement"), dict) else {}
    if sweep_state:
        for key in ("sweep_dir", "sweep_generated_utc", "sweep_config", "sweep_source"):
            meas.pop(key, None)
        if sweep_output:
            meas.update(sweep_output)
    if decode_tps is not None:
        try:
            meas["decode_tps_conc1"] = float(decode_tps)
        except ValueError as exc:
            raise WriterRefusal(f"--measurement-decode-tps 가 수가 아니다: {decode_tps!r}") from exc
        meas["measurement_ok"] = True
    if measurement_source is not None:
        meas["source"] = measurement_source
    meas.setdefault("decode_tps_conc1", None)
    meas.setdefault("measurement_ok", False)
    meas.setdefault("source", None)
    if meas.get("measurement_ok") and not meas.get("source"):
        raise WriterRefusal("측정값을 적으려면 --measurement-source 가 필요하다(출처 표시)")
    # 결손은 **기재**한다 — 차단하지 않는다(사용자 결정 D17: 결정론이 과하면 캠페인이 hang 한다).
    # 부재를 조용히 두면 "안 쟀다"와 "쟀는데 아무도 안 옮겼다"가 구분되지 않는다.
    if gap:
        meas["gap"] = gap
    elif outcome == "measured" and not meas.get("measurement_ok"):
        meas["gap"] = ("outcome=measured 인데 측정값이 도착하지 않았다 — "
                       "--sweep-state 또는 --measurement-decode-tps 미제공(결손 기재)")
    else:
        meas.pop("gap", None)
    doc["measurement"] = meas

    # ── 이미지 축(R5) — 파생 우선, 선언 보조, 모순은 거부 ─────────────────────────────────
    #   **차단하지 않는다**: 결손은 기재하고 진행한다(진행을 막는 자리는 선언 확인 팝업·purge
    #   게이트·발행 게이트 셋뿐이다). 다만 파생값과 선언값이 **다르면** 그것은 결손이 아니라
    #   거짓이므로 거부한다 — 둘 중 하나는 틀렸고, 틀린 좌표는 지도를 조용히 오염시킨다.
    img = doc.get("image") if isinstance(doc.get("image"), dict) else {}
    derived, why = (None, "")
    if sweep_state:
        derived, why = _sweep_image(Path(sweep_state), cell)
    if derived and image_digest and derived["digest"] != image_digest:
        raise WriterRefusal(
            "이미지 좌표가 모순이다 — 선언=%s · sweep 파생=%s (%s). 결손이 아니라 거짓이므로 "
            "기재하지 않는다. 어느 쪽이 이 셀을 실제로 돌린 이미지인지 먼저 가려라."
            % (image_digest, derived["digest"], derived["source"]))
    if derived:
        img.update(derived)
        img.pop("gap", None)
    elif image_digest:
        img.update({"digest": image_digest, "tag": image_tag,
                    "source": "campaign_init --image-digest(선언)"})
        img.pop("gap", None)
    else:
        img.setdefault("digest", None)
        img.setdefault("tag", image_tag)
        img.setdefault("source", None)
        img["gap"] = (why or "이미지 digest 가 도착하지 않았다 — --sweep-state 도 --image-digest 도 "
                             "없다. 셀 간 비교는 같은 digest 안에서만 성립하므로, 이 셀은 "
                             "다른 셀과 나란히 놓을 수 없다(결손 기재).")
    doc["image"] = img

    # ── 셀 출처(2026-09-14 · plan_26091407 §4.0) — **기재만 한다**(차단 ✗ · Q1 "표시+대조") ─────
    #   진입을 막는 자리는 `broad_search.sh cell` precheck 하나이고, 여기는 선언(config.yaml)과 서빙
    #   실물(sweep 좌표)이 갈라진 사실을 남긴다. 승자 셀 yaml 의 gmu 0.80 이 config target_gmu 0.85 와
    #   달랐는데 그 사실이 어떤 산출물에도 없었다(F1). 값은 호출부가 나르지 않고 writer 가 직접 읽는다.
    _prior = doc.get("provenance") if isinstance(doc.get("provenance"), dict) else {}
    _prior_mm = doc.get("provenance_mismatch") if isinstance(doc.get("provenance_mismatch"), list) else []
    doc["provenance"], doc["provenance_mismatch"] = _cell_provenance(
        base, cell, node, sweep_state, _prior, _prior_mm)

    # C3(policy:LIBRARY_GROUNDING_FAIL_CLOSED) — 산출물은 자기 그라운딩을 **스스로 밝힌다**.
    # 표시가 없으면 "참조해서 정했다"와 "그냥 정했다"가 데이터에서 구분되지 않고, 그러면
    # 헌법 불변식 B("인용 없는 결정은 누락")가 집행 불가가 된다(결정론 규율 §출처 표시와 같은 형태).
    _g = latest_grounding(base)
    doc["grounding"] = ({"status": ((_g[1].get("export") or {}).get("resolution") or {}).get("status"),
                         "source": _rel(_g[0]),
                         "gap": (_g[1].get("attestation") or {}).get("grounding_gap")}
                        if _g else {"status": "absent", "source": None,
                                    "gap": {"status": "absent",
                                            "reason": "그라운딩 기록이 없다(C1 미수행)"}})
    doc["void_reason"] = void_reason
    doc["void_reason_source"] = void_reason_source
    lite_verdict = None
    if lite_raw:
        lite_verdict = _apply_lite_gate(doc, lite_raw, utc)
    if axis_citation is not None:
        doc["axis_citation"] = axis_citation
    if next_intent is not None:
        doc["next_intent"] = next_intent
    _write_json(path, doc)

    journey = None
    if next_intent:
        # 여정 한 줄은 **같은 트랜잭션**에 실린다 — 새 절차를 만들지 않고 이미 도는 자동쓰기에
        # 인자 하나를 얹는 방식이다(인터뷰 Q4).
        entry = {"utc": utc, "node_id": node, "cell_id": cell, "outcome": outcome,
                 "axis_citation": axis_citation, "next_intent": next_intent,
                 "source": "campaign_init --cell-set"}
        if lite_raw:
            entry["lite_verdict"] = lite_verdict
        journey = writer_append_journey(base, entry)
    return path, journey


# ── lite ② → cap 차감 · 재발동 제안(2026-09-29 · plan_26092923_58_27) ─────────────────────────────────
#   workflow.md §실패 라우팅의 lite ② 행("실사용 불가 — loop cap −1 · 재발동은 사람 승인 게이트")을 **셀 상태에** 싣는다.
#   · 판정은 다시 하지 않는다 — raw 의 `lite_verdict` 를 소유자 규칙(`lite_metrics.read_lite_verdict`)으로 읽는다.
#   · 차감은 **몇 번 썼나**만 적는다(`reconciliation.charges[]` · 키 = raw measured_utc → 같은 측정은 한 번). cap 값은
#     소유자(explorer `recipe.py --cap`)만 알고, 잔여는 소비자가 `cap − spent` 로 파생한다(같은 개념을 두 자리에 적지 않는다).
#   · ① measurement_path_failed 는 하네스 결함이라 차감하지 않는다(재빌드 ✗ · 하네스 수리).
#   · 재발동은 **제안**까지다. 실행은 사람 승인(대화 평면)을 거치고, 그 결정은 `--reentry-decide` 로 기록한다 — 진행을 막는
#     자리(선언 팝업·purge·발행 게이트)를 늘리지 않는다.
REENTRY_ROUTES = {"explorer": "vllm-recipe-explorer(서빙전략 재수립)",
                  "upstream": "upstream-version-watch(재빌드)"}


def _read_lite_raw(raw_path: str) -> "tuple[str | None, str | None, str]":
    """lite raw → (판정|None, measured_utc|None, 출처). 판정 읽기 규칙은 소유자에게 묻는다(복제 ✗)."""
    import importlib.util
    mod_path = (Path(__file__).resolve().parents[2] / "adversarial-benchmark" / "scripts" / "lite_metrics.py")
    raw = _read_json(Path(raw_path))
    if not isinstance(raw, dict):
        raise WriterRefusal(f"--lite-raw 를 JSON 객체로 읽을 수 없다: {raw_path}")
    if not mod_path.is_file():
        # 물을 데가 없으면 판정 없음이다 — 추측으로 차감하지도, 통과로 접지도 않는다.
        return None, raw.get("measured_utc"), f"not_evaluated(판정 읽기 소유자 부재: {mod_path})"
    spec = importlib.util.spec_from_file_location("_lite_metrics", mod_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return (mod.read_lite_verdict(raw), raw.get("measured_utc"),
            f"lite_raw({_rel(Path(raw_path).resolve())}) · lite_metrics.read_lite_verdict")


def _apply_lite_gate(doc: dict, lite_raw: str, utc: str | None) -> "str | None":
    verdict, measured, source = _read_lite_raw(lite_raw)
    doc["lite_gate"] = {"verdict": verdict, "measured_utc": measured, "source": source}
    if verdict != "server_failed":
        return verdict
    rec = doc.get("reconciliation") if isinstance(doc.get("reconciliation"), dict) else {}
    charges = [c for c in (rec.get("charges") or []) if isinstance(c, dict)]
    key = measured or source
    fresh = not any(c.get("key") == key for c in charges)
    if fresh:
        charges.append({"key": key, "utc": utc, "reason": "lite_verdict=server_failed(실사용 불가)", "source": source})
    doc["reconciliation"] = {
        "charges": charges, "spent": len(charges),
        "cap_owner": "vllm-recipe-explorer recipe.py --cap — 잔여 = cap − spent(cap 값은 여기에 적지 않는다)"}
    prior = doc.get("reentry") if isinstance(doc.get("reentry"), dict) else None
    if fresh or prior is None:
        doc["reentry"] = {
            "status": "proposed", "reason": "lite_verdict=server_failed", "charge_key": key,
            "proposed_utc": utc, "routes": list(REENTRY_ROUTES.values()), "decision": None,
            "gate": "사람 승인(대화 평면) → campaign_init --reentry-decide 로 기록 · 자동 실행 ✗"}
    return verdict


def writer_decide_reentry(base: Path, *, cell: str, decision: str, route: str | None,
                          approved_by: str | None, utc: str | None) -> "tuple[Path, Path]":
    """재발동 제안에 대한 사람의 결정을 **기록**한다. 제안이 없으면 받지 않는다(없는 제안의 승인은 단언이다)."""
    if decision not in ("approve", "decline"):
        raise WriterRefusal(f"--decision 은 approve|decline 이다: {decision!r}")
    if decision == "approve" and route not in REENTRY_ROUTES:
        raise WriterRefusal(f"승인에는 --route {'|'.join(REENTRY_ROUTES)} 가 필요하다(누가 재발동하는가를 먼저 적는다)")
    if not (approved_by or "").strip() or not utc:
        raise WriterRefusal("--reentry-decide 는 --approved-by(사람 발화 전사)와 --utc 가 필요하다")
    path = base / "cells" / cell / "cell.status.json"
    doc = _read_json(path) if path.is_file() else None
    reentry = doc.get("reentry") if isinstance(doc, dict) else None
    if not isinstance(reentry, dict) or reentry.get("status") != "proposed":
        raise WriterRefusal(f"셀 {cell} 에 대기 중인 재발동 제안이 없다(status="
                            f"{(reentry or {}).get('status') if isinstance(reentry, dict) else None!r}) — "
                            "제안은 lite ② 가 셀 상태에 실릴 때 생긴다(--cell-set --lite-raw)")
    reentry["status"] = "approved" if decision == "approve" else "declined"
    reentry["decision"] = {"decision": decision, "route": REENTRY_ROUTES.get(route) if route else None,
                           "approved_by": approved_by.strip(), "utc": utc}
    doc["reentry"] = reentry
    _write_json(path, doc)
    journey = writer_append_journey(base, {
        "utc": utc, "node_id": doc.get("node_id"), "cell_id": cell, "outcome": doc.get("cell_outcome"),
        "reentry": reentry["status"], "route": reentry["decision"]["route"],
        "source": "campaign_init --reentry-decide"})
    return path, journey


ESCALATION_NAME = "escalation_candidates.jsonl"


def judge_escalation_predicate(symptoms, *, events=None, started_utc=None, ended_utc=None) -> dict:
    """술어 ① 을 **소유자에게 묻는다** — 판정 로직을 여기 복제하지 않는다.

    정본은 `vllm-recipe-explorer/scripts/escalation_predicate.py` 이고(사살 어휘의 정본은 다시
    `classify_cell.py` 다), 이 함수는 호출부다. 소유자가 없으면 **추측하지 않고** 그 사실을
    판정에 싣는다 — 없는 판정을 `false` 로 적으면 "① 이 서지 않았다" 와 "물을 데가 없었다" 가
    구분되지 않는다.
    """
    import importlib.util
    mod_path = (Path(__file__).resolve().parents[2] / "vllm-recipe-explorer" / "scripts"
                / "escalation_predicate.py")
    if not mod_path.is_file():
        return {"predicate_1": None, "model_plane_standing": [], "model_plane_refuted": [],
                "host_plane_ignored": [], "unknown_symptoms": list(symptoms or []),
                "evidence_source": "none",
                "reasons": [f"술어 ① 판정기가 없다({mod_path}) — 판정 불가를 false 로 적지 않는다. "
                            f"vllm-recipe-explorer 배달을 확인하라."]}
    spec = importlib.util.spec_from_file_location("_escalation_predicate", mod_path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    hits = []
    scanned = "none"
    if events and started_utc and ended_utc:
        classify = mod._load_classifier()
        lines = []
        for path in events:
            try:
                with open(path, encoding="utf-8") as fh:
                    lines.extend(fh.readlines())
            except OSError:
                continue
        hits = classify.kill_events_in_window(
            lines, classify._utc(started_utc, "--window-start-utc"),
            classify._utc(ended_utc, "--window-end-utc"), 0)
        scanned = ",".join(events)
    return mod.judge(symptoms or [], hits, scanned)


def writer_append_escalation(base: Path, entry: dict) -> Path:
    """escalation 후보 한 줄 (2026-09-11 신설 · plan_26091108 R6). append-only.

    왜 있는가. 헌법은 escalation 역루프를 갖고 SKILL.md 는 `next_strategy_hint` 로 재탐색을
    자극한다고 적었는데, **그 이름을 생성하거나 소비하는 코드가 저장소 전체에 0 건**이었다.
    산문에만 있는 루프는 돌지 않는다 — camp-26090918 은 셀이 12번 죽는 동안 빌드·서빙 전략의
    재수립을 한 번도 제안하지 않았다.

    왜 **원장에 쌓고 종결 시 묻는가**(사용자 결정 · plan 옵션 ii): 빌드 평면까지 건드리는
    재전략은 이미지 재빌드를 뜻하고, 그것을 캠페인 중간에 발동시키면 통제변인이 캠페인 도중
    바뀐다 — 그 뒤의 셀은 앞의 셀과 비교할 수 없게 된다. 그래서 후보는 **쌓기만** 하고 발동은
    종결 시 사람이 판단한다.

    ★ 후보 ≠ escalation. 각 줄은 술어 ①(`escalation_predicate.py`)의 판정을 **함께** 든다 —
      호스트 평면 사인으로 죽은 셀은 후보 목록에 남되 `predicate_1: false` 로 남아, 종결
      HITL 이 "이건 예산 문제지 빌드 문제가 아니다" 를 데이터로 읽는다.
    """
    path = base / ESCALATION_NAME
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n")
    return path


def read_escalations(base: Path) -> list:
    path = base / ESCALATION_NAME
    if not path.is_file():
        return []
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        if isinstance(rec, dict):
            out.append(rec)
    return out


def escalation_summary(base: Path) -> dict:
    """종결 HITL 이 읽는 요약. **판정하지 않는다** — 세어서 보여주고 사람이 정한다."""
    rows = read_escalations(base)
    standing = [r for r in rows if r.get("predicate_1") is True]
    host = [r for r in rows if r.get("predicate_1") is False
            and (r.get("host_plane_ignored") or r.get("model_plane_refuted"))]
    return {
        "campaign_id": base.name,
        "total": len(rows),
        "predicate_1_standing": len(standing),
        "host_plane_only": len(host),
        "cells_standing": sorted({str(r.get("cell_id")) for r in standing}),
        "cells_host_plane": sorted({str(r.get("cell_id")) for r in host}),
        "hitl_required": bool(standing),
        "rows": rows,
        "_note": ("술어 ① 이 선 후보가 하나라도 있으면 캠페인 종결 시 사람에게 묻는다 — 빌드 평면 "
                  "재전략(포크 핀·빌드 패치·이미지 재빌드)은 통제변인을 바꾸므로 캠페인 중간에 "
                  "발동하지 않는다. ① 이 서지 않은 줄은 예산·호스트 평면의 일이며 그 처방은 "
                  "R1~R3 의 게이트가 이미 갖고 있다."),
    }


def writer_append_journey(base: Path, entry: dict) -> Path:
    """여정 줄 append-only. 지도가 영토와 갈라진 지점의 기록이며, 이 체인에서 **유일하게
    복원 불가능한 정보**다(벤치 결과·3+1+1 산출물은 결손 기재로 통과한다)."""
    path = base / JOURNEY_NAME
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n")
    return path


def read_journey(base: Path) -> list:
    path = base / JOURNEY_NAME
    if not path.is_file():
        return []
    out = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except ValueError:
            out.append({"_unparsed": line})
    return out


def _sub_evidence_binding(base: Path, node: str | None, cell_id: str | None) -> None:
    decl = read_declaration(base)
    own = [n for n in (decl.get("nodes") or []) if isinstance(n, dict) and n.get("node_id") == node]
    if not isinstance(node, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", node) \
            or len(own) != 1 or own[0].get("role") != "sub":
        raise WriterRefusal(f"서브 증거 node_id가 선언된 sub가 아니다: {node!r}")
    if cell_id is not None and (not isinstance(cell_id, str)
            or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", cell_id)
            or node_of_cell(base, cell_id) != node):
        raise WriterRefusal(f"서브 증거 셀이 이 노드 배정과 다르다: node={node!r}, cell={cell_id!r}")


def _sub_evidence_path(kind: str, path_rel: str, docs_root: Path) -> Path:
    """One docs-only path contract for the sub writer and upward brief importer."""
    if kind not in EVIDENCE_KINDS or not isinstance(path_rel, str) or not path_rel.startswith("docs/") \
            or ".." in Path(path_rel).parts or Path(path_rel).is_absolute() or len(Path(path_rel).parts) < 3:
        raise WriterRefusal(f"서브 evidence claim은 docs 아래의 유효 kind/상대경로여야 한다: {kind!r}, {path_rel!r}")
    _no_fill(path_rel=path_rel)
    local = docs_root / path_rel.removeprefix("docs/")
    if docs_root.is_symlink() or any(p.is_symlink() for p in (local, *local.parents)
                                   if p != docs_root and docs_root in p.parents):
        raise WriterRefusal(f"서브 evidence 경로에 symlink가 있다: {path_rel}")
    try:
        local.resolve().relative_to(docs_root.resolve())
    except ValueError as exc:
        raise WriterRefusal(f"서브 evidence 경로가 docs/mirror 밖으로 벗어난다: {path_rel}") from exc
    return local


def writer_add_evidence(base: Path, *, kind: str, path_rel: str, cell_id: str | None,
                        node: str | None, unfreeze: bool) -> Path:
    if kind not in EVIDENCE_KINDS:
        raise WriterRefusal(f"kind 는 {EVIDENCE_KINDS} 중 하나여야 한다: {kind!r}")
    _no_fill(path_rel=path_rel)
    if not node and cell_id:
        node = node_of_cell(base, cell_id)
    declaration = read_declaration(base)
    declared = [n for n in (declaration.get("nodes") or []) if isinstance(n, dict)]
    if declaration.get("self_role") == "sub":
        if not node and len(declared) == 1:
            node = declared[0].get("node_id")
        _sub_evidence_binding(base, node, cell_id)
        local = _sub_evidence_path(kind, path_rel, REPO_ROOT / "docs")
        if not local.is_file() and not (kind == "simlog" and local.is_dir()):
            raise WriterRefusal(f"서브 증거는 실재 정규파일 또는 simlog 디렉터리여야 한다: {path_rel}")
    if not (REPO_ROOT / path_rel).exists():
        # 증거는 docs 평면에서 **태어난다** — 포인터가 가리키는 것이 없으면 그것은 포인터가 아니다.
        raise WriterRefusal(f"증거가 실재하지 않는다: {path_rel} "
                            f"(증거는 docs 평면에서 태어나고 여기엔 포인터만 둔다)")
    if len(declared) > 1 and not node:
        # ★ 2026-09-08: camp-26090721 의 포인터 6건이 **전부** node_id null 이었다. 그래서 P2 는
        #   검사할 노드를 하나도 찾지 못한 채 통과했다 — 게이트가 있는데 아무것도 안 물은 것이다.
        raise WriterRefusal(
            f"--node 가 필요하다 — 이 캠페인은 노드가 {len(declared)}개다. 노드 태그 없는 증거는 "
            f"'어느 노드가 낸 것인지 모르는 증거'이고, P2 는 그런 포인터를 검사하지 못해 공허하게 "
            f"통과한다(2026-09-08 실측). 셀을 함께 주면(--cell) 배정에서 파생한다.")
    ep = base / "evidence_pointers.json"
    doc = _read_json(ep) if ep.is_file() else None
    doc = doc if isinstance(doc, dict) else {"schema_version": 1, "pointers": []}
    if doc.get("frozen_utc") and not unfreeze:
        # ★ 2026-09-21(plan_26092119 D9): 동결은 사람이 명시로 건 **캠페인 전체** 스냅샷이다 — hint 발행기는
        #   셀 단위로 발행하므로 동결하지 않고, 그 셀 몫 포인터를 봉인 페이로드에 복사해 '태그는 불변인데
        #   근거가 움직인다'(2026-09-07 인터뷰 Q6)를 페이로드 쪽에서 막는다.
        raise WriterRefusal(
            f"이 스냅샷은 {doc['frozen_utc']} 에 동결됐다 — 동결 이후에는 입력이 바뀌지 않는다"
            f"(사람이 명시로 건 캠페인 전체 스냅샷이다).\n"
            f"  → 정말 바꿔야 하면 사람이 --unfreeze 를 붙인다.")
    pointers = doc.get("pointers")
    pointers = pointers if isinstance(pointers, list) else []
    for ptr in pointers:
        if isinstance(ptr, dict) and ptr.get("path") == path_rel and ptr.get("kind") == kind:
            # 멱등 — 같은 포인터를 두 번 적지 않는다. 다만 **비어 있는 태그는 채운다**:
            # 종전 writer 는 null 로 적힌 node_id 를 고칠 연산이 없어서, 태그를 붙이려면 손으로
            # 파일을 여는 수밖에 없었다(F1 과 같은 형태 — 연산이 없으면 손이 들어온다).
            # 값이 이미 있고 **다른 값을 주면 거부**한다: 조용한 덮어쓰기는 증거의 귀속을 바꾼다.
            changed = False
            for key, val in (("node_id", node), ("cell_id", cell_id)):
                if val is None:
                    continue
                have = ptr.get(key)
                if have in (None, ""):
                    ptr[key] = val
                    changed = True
                elif have != val:
                    raise WriterRefusal(
                        f"이 포인터의 {key} 는 이미 {have!r} 다 — {val!r} 로 바꾸려면 사람이 "
                        f"판단해야 한다(증거의 귀속을 조용히 바꾸지 않는다): {path_rel}")
            if changed:
                doc["pointers"] = pointers
                _write_json(ep, doc)
            return ep
    pointers.append({"kind": kind, "path": path_rel, "cell_id": cell_id, "node_id": node})
    doc["pointers"] = pointers
    doc.setdefault("campaign_id", base.name)
    _write_json(ep, doc)
    return ep


def writer_untag_evidence(base: Path, *, kind: str, path_rel: str, cell_id: str, reason: str,
                          utc: str, unfreeze: bool) -> Path:
    """포인터의 셀 귀속(cell_id)을 해제한다 — 포인터는 남기고 이력(`untagged[]`)에 옛 귀속·사유·시각을 적는다.

    왜(2026-09-28 · camp-26092808 · 사용자 승인): `--evidence-add` 는 태그 부재를 채우고 다른 값은 거부한다(조용한
    귀속 변경 ✗). 그런데 **잘못 붙인 셀 태그를 떼는 연산이 없었다** — 셀 선행 근거(모델리스 testlog)에 셀 태그를 붙이자
    hint 발행기가 `HINT_NARRATIVE_EVIDENCE_AMBIGUOUS` 로 멈췄고, 남은 길은 상태 파일 손편집뿐이었다(연산이 없으면 손이
    들어온다 · F1 과 같은 형태). 해제는 귀속을 **다른 셀로 옮기지 않는다**(재귀속은 add 가 빈 태그를 채우는 경로) ·
    사유·시각 없는 해제는 거부 · 기대한 셀과 다르면 거부.
    """
    if not reason or not str(reason).strip() or not utc:
        raise WriterRefusal("--evidence-untag 는 --untag-reason 과 --utc 가 필요하다(사유 없는 귀속 해제는 드리프트다)")
    ep = base / "evidence_pointers.json"
    doc = _read_json(ep) if ep.is_file() else None
    if not isinstance(doc, dict):
        raise WriterRefusal(f"증거 포인터 파일이 없다: {_rel(ep)}")
    if doc.get("frozen_utc") and not unfreeze:
        raise WriterRefusal(f"이 스냅샷은 {doc['frozen_utc']} 에 동결됐다 — 사람이 --unfreeze 를 붙여야 한다")
    hits = [x for x in (doc.get("pointers") or [])
            if isinstance(x, dict) and x.get("path") == path_rel and x.get("kind") == kind]
    if len(hits) != 1:
        raise WriterRefusal(f"(kind={kind!r}, path={path_rel!r}) 포인터가 정확히 1건이 아니다: {len(hits)}건")
    ptr = hits[0]
    if ptr.get("cell_id") != cell_id:
        raise WriterRefusal(f"이 포인터의 cell_id 는 {ptr.get('cell_id')!r} 다 — --cell {cell_id!r} 와 다르다(엉뚱한 해제 ✗)")
    ptr.setdefault("untagged", []).append({"cell_id": cell_id, "reason": str(reason).strip(), "utc": utc})
    ptr["cell_id"] = None
    _write_json(ep, doc)
    return ep


def writer_relocate_evidence(base: Path, *, kind: str, path_from: str, path_to: str, reason: str,
                             utc: str, unfreeze: bool, correct_reference: bool = False,
                             node: str | None = None, cell_id: str | None = None) -> Path:
    """포인터의 경로를 옮긴다 — 문서가 개명됐을 때(명명 SSOT 로 다시 지은 이름) 포인터를 따라 옮기는 정식 연산.

    왜(2026-09-29 · camp-26092913 · plan_26092908 S10): 발행 직전 판정 testlog·devlog·simlog 를 명명 SSOT 와 다른 시각(KST 시)으로
    지었다가 바로잡았는데, writer 에는 추가·귀속 해제·스텁 제거만 있고 **경로를 옮기는 연산이 없었다** — 옛 경로 포인터는 purge
    게이트(전수 실재)를 막고, 남은 길은 상태 파일 손편집뿐이었다(연산이 없으면 손이 들어온다 · untag 와 같은 형태).
    기본 옮김은 증거를 지우지 않는다: 옛 경로에 파일이 **없고** 새 경로에 파일이 **있을** 때만(개명의 관측) · 같은 종류의 새 경로
    포인터가 이미 있으면 거부(중복) · 셀 귀속은 그대로 · 사유·시각 없는 옮김은 거부 · 이력은 `relocated[]`.
    명시적 correct_reference는 sub relay_summary의 잘못된 제어원장 참조를 docs/testlog 요약으로 정정한다.
    이 경우에만 원본 relay 파일의 존속을 요구하며 node/cell 귀속과 정정 이력을 함께 검증한다.
    """
    if not reason or not str(reason).strip() or not utc:
        raise WriterRefusal("--evidence-relocate 는 --relocate-reason 과 --utc 가 필요하다")
    ep = base / "evidence_pointers.json"
    doc = _read_json(ep) if ep.is_file() else None
    if not isinstance(doc, dict):
        raise WriterRefusal(f"증거 포인터 파일이 없다: {_rel(ep)}")
    if doc.get("frozen_utc") and not unfreeze:
        raise WriterRefusal(f"이 스냅샷은 {doc['frozen_utc']} 에 동결됐다 — 사람이 --unfreeze 를 붙여야 한다")
    if Path(path_to).is_absolute() or ".." in Path(path_to).parts:
        raise WriterRefusal(f"새 경로는 저장소 상대여야 한다: {path_to!r}")
    if (REPO_ROOT / path_from).exists() and not correct_reference:
        raise WriterRefusal(f"옛 경로가 아직 실재한다({path_from}) — 개명이 아니다(증거를 옮기지 않는다)")
    if not (REPO_ROOT / path_to).exists():
        raise WriterRefusal(f"새 경로가 실재하지 않는다: {path_to}")
    ptrs = [x for x in (doc.get("pointers") or []) if isinstance(x, dict)]
    hits = [x for x in ptrs if x.get("path") == path_from and x.get("kind") == kind]
    if len(hits) != 1:
        raise WriterRefusal(f"(kind={kind!r}, path={path_from!r}) 포인터가 정확히 1건이 아니다: {len(hits)}건")
    if any(x.get("path") == path_to and x.get("kind") == kind for x in ptrs):
        raise WriterRefusal(f"(kind={kind!r}, path={path_to!r}) 포인터가 이미 있다 — 중복 ✗")
    ptr = hits[0]
    history = {"from": path_from, "reason": str(reason).strip(), "utc": utc}
    if correct_reference:
        # A relay ledger is retained locally; its upward evidence is a separate docs summary.
        # This is an explicit reference correction, never a claim that the source was renamed.
        if kind != "relay_summary" or read_declaration(base).get("self_role") != "sub" or not node or not cell_id:
            raise WriterRefusal("--correct-reference 는 sub relay_summary 정정이며 --node --cell이 필요하다")
        _sub_evidence_binding(base, node, cell_id)
        if ptr.get("node_id") != node or ptr.get("cell_id") != cell_id:
            raise WriterRefusal("정정할 포인터의 node/cell 귀속이 --node/--cell과 다르다")
        source = REPO_ROOT / path_from
        if Path(path_from).is_absolute() or ".." in Path(path_from).parts or not source.is_file() \
                or source.is_symlink() or source.parent.resolve() != (base / "relay").resolve():
            raise WriterRefusal("정정 원본은 이 캠페인의 보존할 relay 정규파일이어야 한다")
        target = _sub_evidence_path(kind, path_to, REPO_ROOT / "docs")
        if not path_to.startswith("docs/testlog/") or not target.is_file() or target.suffix != ".md":
            raise WriterRefusal("정정 목적지는 실제 docs/testlog Markdown 요약 정규파일이어야 한다")
        history.update(mode="reference-correction", source_preserved=True, node_id=node, cell_id=cell_id)
    elif read_declaration(base).get("self_role") == "sub":
        _sub_evidence_binding(base, ptr.get("node_id"), ptr.get("cell_id"))
        _sub_evidence_path(kind, path_to, REPO_ROOT / "docs")
    ptr.setdefault("relocated", []).append(history)
    ptr["path"] = path_to
    _write_json(ep, doc)
    return ep


def writer_prune_stubs(base: Path, *, unfreeze: bool) -> "tuple[Path, list[str]]":
    """뼈대에서 딸려온 `<<FILL>>` 스텁 포인터를 정식 경로로 지운다(2026-09-08 · plan_26090813).

    왜 writer 에 지우는 연산이 필요한가: 종전 writer 는 **append 만** 했다. 그래서 뼈대의 예시
    포인터가 인스턴스로 복사돼 purge 게이트를 막았을 때, 그것을 치울 정식 경로가 없어 사람이
    손으로 지웠다 — 그리고 손삭제의 흔적은 "채워야 했는데 못 채운 빈칸"과 구분되지 않는다
    (2026-09-08 사후감사 §A F1). D3: 우회하지 말고 경로를 만든다.

    지우는 대상은 **빈칸이 남은 항목뿐**이다. 값이 든 포인터는 건드리지 않는다 — 증거를 지우는
    연산이 아니라 틀 잔재를 치우는 연산이다. 무엇을 지웠는지 돌려주고 호출부가 출력한다
    (침묵 삭제 금지 · docs.md 선례).
    """
    ep = base / "evidence_pointers.json"
    doc = _read_json(ep)
    if not isinstance(doc, dict):
        raise WriterRefusal("evidence_pointers.json 이 없거나 파손")
    if doc.get("frozen_utc") and not unfreeze:
        raise WriterRefusal(f"이 스냅샷은 {doc['frozen_utc']} 에 동결됐다 — 사람이 --unfreeze 를 붙인다")
    kept, dropped = [], []
    for ptr in (doc.get("pointers") or []):
        blob = json.dumps(ptr, ensure_ascii=False)
        (dropped if FILL in blob else kept).append(blob)
    doc["pointers"] = [json.loads(x) for x in kept]
    if doc.get("campaign_id") == FILL:
        doc["campaign_id"] = base.name        # 부재가 아니라 빈칸이라 setdefault 가 못 덮었다
    _write_json(ep, doc)
    return ep, dropped


def writer_freeze_evidence(base: Path, utc: str) -> Path:
    """증거 스냅샷을 **캠페인 전체** 단위로 동결한다(인터뷰 Q6 · 2026-09-07).

    ★ 2026-09-21(plan_26092119 D9 · 코드맵 H9): hint 발행기는 이 연산을 부르지 않는다. 발행이 셀 단위가
      되면서, 첫 셀의 발행이 전체를 얼리면 이후 셀의 증거 등록(publish_benchmark_record 의 인증서 포인터
      포함)이 전부 거부된다 — 캠페인이 도는 중에 발행이 캠페인을 막는다. 태그 불변 요구는 발행기가 그
      셀 몫 포인터를 봉인 페이로드(PAYLOAD.evidence_pointers)에 복사해서 진다. 이 연산은 사람이 명시로
      쓰는 전체 동결로 남는다(호출자 0 · 종전 문구 'publish 위상의 proof.ok 시점' 은 배선된 적이 없다)."""
    ep = base / "evidence_pointers.json"
    doc = _read_json(ep) if ep.is_file() else None
    if not isinstance(doc, dict):
        raise WriterRefusal("evidence_pointers.json 이 없거나 파손 — 동결할 스냅샷이 없다")
    doc["frozen_utc"] = utc
    _write_json(ep, doc)
    return ep


def writer_add_revision(base: Path, *, values: dict, reason: str, evidence: str,
                        approved_by: str, utc: str) -> Path:
    """layer-1 control_variables 의 append-only 개정(인터뷰 Q3 · 메인 단일 창구).

    revisions[0] 은 t0 원본이며 **불변**이다. 대조 기준은 실행에서 파생돼 자기일치하는
    work-manifest 가 아니라 이 원본이다.
    """
    for need, val in (("--revise-reason", reason), ("--revise-evidence", evidence),
                      ("--revise-approved-by", approved_by)):
        if not val:
            raise WriterRefusal(f"{need} 는 필수다 — 사유·근거·승인자 없는 개정은 개정이 아니다")
    path = base / "campaign.yaml"
    doc = _read_json(path)
    if not isinstance(doc, dict):
        raise WriterRefusal("campaign.yaml 이 없거나 파손")
    revs = doc.get("revisions")
    revs = revs if isinstance(revs, list) else []
    if not revs:
        base_vals = doc.get("control_variables") or {}
        revs.append({"rev": 0, "declared_utc": doc.get("declared_utc"), "values": base_vals,
                     "reason": "t0 선언", "evidence": doc.get("plan_ref"),
                     "approved_by": "t0"})
    merged = dict(revs[-1].get("values") or {})
    merged.update(values)
    revs.append({"rev": len(revs), "declared_utc": utc, "values": merged, "reason": reason,
                 "evidence": evidence, "approved_by": approved_by})
    doc["revisions"] = revs
    # control_variables 는 **t0 원본으로 남긴다** — 최신값은 revisions[-1] 이 든다.
    _write_json(path, doc)
    return path


def writer_hint_approve(base: Path, *, node: str | None, cells: list, approved_by: str | None,
                        utc: str | None, source: str | None = None) -> "tuple[Path, dict]":
    """hint 발행 **사전승인**을 선언의 `hint_targets` 에 전사한다(2026-09-21 · plan_26092119 O6 · SPEC X3).

    왜: O6 는 셀마다 팝업 대신 선언 확인 팝업에서 발행 대상 셀 목록을 승인하기로 했는데, 그 승인은 산문에만
    있고 **기계 기록이 0** 이었다(코드맵 G3) — 기록 없는 승인은 세션과 함께 사라지고, 발행기는 사람에게
    다시 묻거나 묻지 않고 발행하는 둘 중 하나가 된다. 무인 자동 태깅 ✗ 는 그대로다: 사람이 명시한 셀
    목록과 그 발화의 전사를 적는 것이지 기계가 셀을 고르는 것이 아니다.

    규율(`--revise` 와 같은 4필드): node · cells(명시 1개 이상) · approved_by(사람 발화 전사) · utc 넷 중 하나라도
    없으면 승인이 아니라 드리프트다. 모양의 판정은 검증기(`hint_target_reasons`)가 소유하고, 이 writer 는 쓰기
    **전에** 그 판정을 불러 검증기가 거부할 선언을 만들지 않는다.

    항목 하나 = 승인 사건 하나(노드 · 전사 · 시각 · 출처가 같으면 같은 사건). 같은 사건을 다시 적으면 셀을
    합치고(멱등), 다른 사건으로 이미 승인된 셀은 **옛 승인을 지킨다** — 귀속을 조용히 바꾸면 그 셀의 승인이
    누구의 어느 발화였는지가 사라진다(writer_add_evidence 의 귀속 불변 선례 · 2026-09-08).

    돌려주는 것: (경로, {"added", "kept", "already", "changed", "index"}).
    """
    v = validator()
    source = source or v.HINT_APPROVAL_DEFAULT_SOURCE
    for need, val in (("--node", node), ("--approved-by", approved_by), ("--utc", utc)):
        if not isinstance(val, str) or not val.strip():
            raise WriterRefusal(f"{need} 는 필수다 — node·cells·approved-by·utc 넷 중 하나라도 없으면 "
                                f"승인이 아니라 드리프트다(--revise 와 같은 규율)")
    wanted: list[str] = []
    for c in (cells or []):
        c = str(c).strip()
        if c and c not in wanted:
            wanted.append(c)
    if not wanted:
        raise WriterRefusal("--cells 는 필수다 — 승인할 셀을 명시 열거한다(빈 목록 = 전 셀 해석은 폐기 · "
                            "백지 승인 ✗)")
    _no_fill(node=node, approved_by=approved_by, utc=utc, source=source, cells=",".join(wanted))
    approval = {"approved_by": approved_by, "approved_utc": utc, "source": source}
    why = v.hint_approval_reason(approval)
    if why is not None:
        raise WriterRefusal(f"승인이 완결되지 않았다 — {why}")

    path = base / "campaign.yaml"
    doc = _read_json(path)
    if not isinstance(doc, dict):
        raise WriterRefusal("campaign.yaml 이 없거나 파손 — 승인을 적을 선언이 없다")
    if doc.get("self_role") == "sub":
        # 2026-10-11(plan_26101104 Step 3): single 서브가 자기 셀을 발행해도 **승인을 적는 자리는 메인 선언 하나**다 —
        #   메인에서 `--hint-approve --node <sub 노드>` 로 적으면 `--emit-slice <sub 노드>` 가 그 몫을 운반한다.
        #   서브가 자기 인스턴스에 승인을 쓰면 사람 발화의 전사가 메인 선언 밖에서 생긴다(자기 승인).
        raise WriterRefusal("self_role=sub 인스턴스다 — 발행 승인은 메인 선언에만 적는다. 서브 셀이면 메인에서 "
                            "`--hint-approve --node <그 서브 노드>` 로 적고, 파생 선언(--emit-slice)이 그 노드 몫을 운반한다")
    if node not in [n.get("node_id") for n in (doc.get("nodes") or []) if isinstance(n, dict)]:
        raise WriterRefusal(f"nodes[] 에 {node!r} 가 없다 — 선언되지 않은 노드의 셀은 승인할 수 없다")
    assigned = v.assigned_cells(doc, node)
    outside = [c for c in wanted if c not in assigned]
    if outside:
        raise WriterRefusal(f"--cells 의 {outside} 는 assignments[{node!r}] 에 없다(배정: {assigned}) — 배정 SSOT 는 "
                            f"assignments 이고 hint_targets 는 파생이다(배정의 노드가 승인의 노드다)")
    before = v.hint_target_reasons(doc)
    if before:
        raise WriterRefusal("선언의 hint_targets 에 이미 결함이 있다 — 결함 위에 승인을 얹지 않는다. 먼저 고친다:\n"
                            + "\n".join(f"  - {b}" for b in before))

    targets = doc.get("hint_targets")
    targets = targets if isinstance(targets, list) else []
    approved_at = {c: i for i, t in enumerate(targets) for c in (t.get("cells") or [])}
    same = next((i for i, t in enumerate(targets)
                 if t.get("node_id") == node and t.get("approval") == approval), None)
    added = [c for c in wanted if c not in approved_at]
    already = [c for c in wanted if c in approved_at and approved_at[c] == same]
    kept = [{"cell": c, "index": approved_at[c], "approval": targets[approved_at[c]].get("approval")}
            for c in wanted if c in approved_at and approved_at[c] != same]
    index = same
    if added:
        if same is not None:
            targets[same]["cells"] = list(targets[same]["cells"]) + added
        else:
            targets.append({"node_id": node, "cells": added, "approval": approval})
            index = len(targets) - 1
        doc["hint_targets"] = targets
        after = v.hint_target_reasons(doc)
        if after:
            # 위 사전 검사가 막았어야 한다 — 여기 오면 writer 와 검증기가 갈라진 것이다. 쓰지 않는다.
            raise WriterRefusal("승인 기록이 검증기를 통과하지 못한다(writer↔검증기 갈라짐 — 쓰지 않았다):\n"
                                + "\n".join(f"  - {x}" for x in after))
        _write_json(path, doc)
    return path, {"added": added, "kept": kept, "already": already, "changed": bool(added), "index": index}


# ─────────────────────────────────────────────────────────────────────────────
# 읽는 눈 — resume-brief (2026-09-07 · plan_26090715 §4.4)

def _short(text, limit: int = 160) -> str:
    """브리핑 표시용 절삭. 파일 내용은 온전하다 — **한 화면**이 아니면 읽는 눈이 되지 못한다."""
    t = " ".join(str(text or "").split())
    return t if len(t) <= limit else t[:limit - 1] + "…"


def _parse_assign_cells(spec: str) -> list:
    """`c1,c2:HITL,c3:STAY` → assignments 항목. 생략된 mode 는 적지 않는다(생략 = AUTO 는 검증기 소유)."""
    out = []
    for tok in [t.strip() for t in (spec or "").split(",") if t.strip()]:
        cell, _, mode = tok.partition(":")
        item = {"cell": cell.strip()}
        if mode:
            if mode not in validator().ASSIGNMENT_MODES:
                raise WriterRefusal(f"--assign-cells 의 mode 는 {validator().ASSIGNMENT_MODES} 중 하나다: {tok!r}")
            item["mode"] = mode
        out.append(item)
    return out


def _cell_lock_reason(base: Path, cell: str) -> "str | None":
    """배정을 바꿀 수 없는 셀인가(K8 — pending 셀만 바꾼다). 돌았거나 도는 셀을 빼면 증거가 배정을 잃는다."""
    st = _read_json(base / "cells" / cell / "cell.status.json")
    if isinstance(st, dict):
        oc = st.get("cell_outcome")
        # materialize 직후의 틀 값(<<FILL>>)은 '아직 아무도 적지 않았다' 이지 판정이 아니다(2026-10-17 라이브 L5 — 돌지 않은
        # 셀이 잠겼다).
        if oc and FILL not in str(oc) and oc not in CELL_OUTCOMES_PENDING:
            return f"cell_outcome={oc}"
        meas = st.get("measurement") if isinstance(st.get("measurement"), dict) else {}
        if meas.get("decode_tps_conc1") is not None:
            return "측정값이 기록됐다"
        if st.get("void_reason"):
            return "사유가 기록됐다"
    phases = base / "phases"
    for ndir in sorted(phases.iterdir()) if phases.is_dir() else []:
        if not ndir.is_dir() or ndir.name.startswith("_"):
            continue
        for ph in PHASE_NAMES:
            doc = _read_json(ndir / f"{ph}.status.json")
            if isinstance(doc, dict) and doc.get("cell_id") == cell and doc.get("state") in ("running", "done"):
                return f"phase {ndir.name}:{ph}={doc.get('state')}"
    return None


def writer_assign_set(base: Path, *, node: str, items: list, reason: str, evidence: str,
                      approved_by: str, utc: str, source: str = "campaign_init --assign-set") -> "tuple[Path, dict]":
    """`assignments[node]` 개정(2026-10-17 · plan_26101722 K8 · `--revise` 와 같은 4필드 규율).

    왜: 배정을 개설 뒤에 바꾸는 writer 가 없어서 변경은 campaign.yaml 손편집이라는 우회로 갔다(D3). 바꿀 수 있는 것은
    **pending 셀뿐**이다 — 돌았거나 도는 셀을 빼거나 mode 를 바꾸면 증거·진행표가 배정을 잃는다. 노드 간 이동은 제거+추가이고,
    다른 노드에 이미 있는 셀을 넣으면 거부한다(이중 배정). hint 승인을 든 셀은 뺄 수 없다(승인 재수령이 먼저 · 무인 재태깅 ✗).
    `assignment_revisions[0]` 은 t0 원본이며 불변이다."""
    for need, val in (("--assign-reason", reason), ("--assign-evidence", evidence),
                      ("--assign-approved-by", approved_by), ("--utc", utc)):
        if not val:
            raise WriterRefusal(f"{need} 는 필수다 — 사유·근거·승인자·시각 없는 배정 변경은 드리프트다")
    path = base / "campaign.yaml"
    doc = read_declaration(base)
    if not doc:
        raise WriterRefusal("campaign.yaml 이 없거나 파손")
    v = validator()
    if node not in {n.get("node_id") for n in (doc.get("nodes") or []) if isinstance(n, dict)}:
        raise WriterRefusal(f"nodes[] 에 {node!r} 가 없다")
    old = v.assignment_items(doc, node)
    old_mode = {i.get("cell"): i.get("mode", v.DEFAULT_MODE) for i in old}
    new_cells = [i["cell"] for i in items]
    if len(set(new_cells)) != len(new_cells):
        raise WriterRefusal(f"--assign-cells 에 같은 셀이 두 번 있다: {new_cells}")
    for cell in new_cells:
        if cell in v.RESERVED_CELL_NAMES or not cell or FILL in cell:
            raise WriterRefusal(f"셀 이름으로 쓸 수 없다: {cell!r}")
        other = next((n for n in v.assignments_of(doc) if n != node and cell in v.assigned_cells(doc, n)), None)
        if other:
            raise WriterRefusal(f"셀 {cell!r} 은 노드 {other!r} 에 배정돼 있다 — 이동은 그 노드에서 먼저 빼고 넣는다"
                                "(제거+추가 · pending 일 때만)")
    removed = [c for c in old_mode if c not in new_cells]
    changed = [i["cell"] for i in items if i["cell"] in old_mode
               and i.get("mode", v.DEFAULT_MODE) != old_mode[i["cell"]]]
    locked = [(c, _cell_lock_reason(base, c)) for c in removed + changed]
    locked = [(c, r) for c, r in locked if r]
    if locked:
        raise WriterRefusal("pending 이 아닌 셀은 빼거나 mode 를 바꿀 수 없다(증거 연결 보존): "
                            + "; ".join(f"{c}({r})" for c, r in locked))
    approved = [c for c in removed for t in (doc.get("hint_targets") or [])
                if isinstance(t, dict) and c in (t.get("cells") or []) and t.get("approval")]
    if approved:
        raise WriterRefusal(f"hint 사전승인을 든 셀은 뺄 수 없다: {sorted(set(approved))} — 승인을 먼저 다시 받는다"
                            "(무인 재태깅 ✗)")
    revs = doc.get("assignment_revisions") if isinstance(doc.get("assignment_revisions"), list) else []
    if not revs:
        revs.append({"rev": 0, "declared_utc": doc.get("declared_utc"), "assignments": v.assignments_of(doc),
                     "reason": "t0 선언", "evidence": doc.get("plan_ref"), "approved_by": "t0"})
    assigns = json.loads(json.dumps(v.assignments_of(doc), ensure_ascii=False))
    assigns[node] = items
    trial = dict(doc, assignments=assigns)
    tmp = path.with_name(".assign_trial.campaign.yaml")
    tmp.write_text(json.dumps(trial, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    try:
        # 셀 입력은 아직 없을 수 있다(배정이 먼저고 materialize 가 뒤다 — 배정 밖 셀은 만들지 않으므로). 이 변경이 **새로**
        # 만든 결함만 막는다 — 이미 있던 선언 결함을 배정 변경의 거부 사유로 삼으면 배정 writer 가 엉뚱한 자리에서 선다.
        before = set(v.validate_campaign(path, strict_paths=False))
        why = [w for w in (x.replace(tmp.name, path.name) for x in v.validate_campaign(tmp, strict_paths=False))
               if w not in before]
    finally:
        tmp.unlink(missing_ok=True)
    if why:
        raise WriterRefusal("배정 변경 뒤 선언이 계약을 깬다:\n" + "\n".join(f"  - {w}" for w in why))
    rev = {"rev": len(revs), "declared_utc": utc, "node": node, "assignments": assigns, "removed": removed,
           "added": [c for c in new_cells if c not in old_mode], "reason": reason, "evidence": evidence,
           "approved_by": approved_by, "source": source}
    revs.append(rev)
    doc["assignments"] = assigns
    doc["assignment_revisions"] = revs
    path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    writer_append_journey(base, _lifecycle_event(
        "campaign_assignments_revised", utc, node=node, rev=rev["rev"], cells=new_cells,
        removed=removed, added=rev["added"], approved_by=approved_by, source=source,
        sub_target=node in _sub_nodes(base)))
    return path, rev


def _slice_revision_extension(decl: dict, sd: dict) -> list:
    """Validate a main-derived control history against the sub's immutable prefix."""
    for key in ("id", "plan_ref", "declared_utc", "control_variables", "derived_from"):
        if decl.get(key) != sd.get(key):
            raise WriterRefusal(f"slice-update control identity mismatch: {key}")
    if not isinstance(decl.get("control_variables"), dict):
        raise WriterRefusal("slice-update original control variables are missing")
    if not isinstance(decl.get("derived_from"), str) or not decl["derived_from"].strip():
        raise WriterRefusal("slice-update main origin is missing")
    local = decl.get("revisions", [])
    incoming = sd.get("revisions")
    if not isinstance(local, list) or not isinstance(incoming, list):
        raise WriterRefusal("slice-update revision history is missing or malformed")
    if not local and not incoming:
        return []
    t0 = {"rev": 0, "declared_utc": decl.get("declared_utc"), "values": decl["control_variables"],
          "reason": "t0 선언", "evidence": decl.get("plan_ref"), "approved_by": "t0"}
    if not isinstance(t0["declared_utc"], str) or not t0["declared_utc"].strip() or not isinstance(t0["evidence"], str) or not t0["evidence"].strip():
        raise WriterRefusal("slice-update original revision identity is incomplete")
    def validate(history):
        if not history:
            return
        if history[0] != t0:
            raise WriterRefusal("slice-update t0 revision conflicts with original declaration")
        previous = t0["values"]
        for number, item in enumerate(history[1:], 1):
            if (not isinstance(item, dict) or set(item) != {"rev", "declared_utc", "values", "reason", "evidence", "approved_by"}
                    or type(item.get("rev")) is not int or item["rev"] != number
                    or not isinstance(item.get("values"), dict) or not set(previous).issubset(item["values"])
                    or any(not isinstance(item.get(k), str) or not item[k].strip()
                           for k in ("declared_utc", "reason", "evidence", "approved_by"))
                    or item["approved_by"] == "t0"):
                raise WriterRefusal("slice-update malformed or unapproved control revision")
            previous = item["values"]
    validate(local)
    validate(incoming)
    if len(incoming) < len(local) or incoming[:len(local)] != local:
        raise WriterRefusal("slice-update control history conflicts or downgrades the existing prefix")
    return incoming[len(local):]


def slice_update(source: str, *, utc, apply: bool) -> list:
    """서브 자기 인스턴스의 배정을 메인 파생 선언으로 바꾼다(K8 · relay `reslice` 의 실행부). pending 규칙은 같은 writer 가
    서브 자기 상태로 다시 검사한다 — 메인이 본 상태와 서브 실물이 갈라졌으면 여기서 거부된다(우회 ✗ · 보고)."""
    if not utc:
        raise WriterRefusal("--slice-update 는 --utc 가 필요하다")
    sd = _load_slice(source)
    if sd.get("self_role") != "sub" or sd.get("authored_by") != SLICE_AUTHOR:
        raise WriterRefusal("--slice-update 는 메인 파생 선언(self_role=sub · authored_by=main-derived)만 받는다")
    camp = sd.get("id")
    base = campaigns_dir() / str(camp)
    decl = read_declaration(base)
    if not decl:
        raise WriterRefusal(f"인스턴스가 없다: campaigns/{camp} — 먼저 메인 파생 선언으로 개설(--init --from-slice)")
    if decl.get("self_role") != "sub":
        raise WriterRefusal(f"campaigns/{camp} 은 서브 자기 인스턴스가 아니다 — 메인 배정은 --assign-set 으로 바꾼다")
    nodes = list((sd.get("assignments") or {}).keys())
    own = [n.get("node_id") for n in (decl.get("nodes") or []) if isinstance(n, dict)]
    if len(nodes) != 1 or nodes[0] not in own:
        raise WriterRefusal(f"선언의 배정 노드 {nodes} 가 이 인스턴스의 노드 {own} 가 아니다(오배달 방지)")
    if sd.get("nodes") != decl.get("nodes") or any(n.get("role") != "sub" for n in sd.get("nodes", []) if isinstance(n, dict)):
        raise WriterRefusal("slice-update node identity or role differs")
    extension = _slice_revision_extension(decl, sd)
    meta = sd.get("_assignment_revision") or {}
    guard_writer_ops(base, {"assign_set": True})
    raw_items = sd["assignments"][nodes[0]]
    if not isinstance(raw_items, list) or any(not isinstance(i, dict) for i in raw_items):
        raise WriterRefusal("slice-update assignment list is malformed")
    items = raw_items
    changed = validator().assignment_items(decl, nodes[0]) != items
    if changed and any(not isinstance(meta.get(k), str) or not meta[k].strip()
                       for k in ("reason", "evidence", "approved_by", "declared_utc")):
        raise WriterRefusal("slice-update assignment change lacks main approval evidence")
    if not apply:
        return [f"dry-run slice-update {camp}@{nodes[0]} → {[i.get('cell') for i in items]}; control revisions +{len(extension)}"]
    if changed:
        _p, rev = writer_assign_set(base, node=nodes[0], items=items,
                                    reason=meta["reason"], evidence=meta["evidence"],
                                    approved_by=f"main-a2a: {meta['approved_by']}", utc=utc,
                                    source=f"campaign_init --slice-update (main rev {meta.get('rev')})")
    else:
        rev = None
    if extension:
        current = read_declaration(base)
        current["revisions"] = list(current.get("revisions") or []) + extension
        _write_json(base / "campaign.yaml", current)
    return [f"slice-update {camp}@{nodes[0]} assignment rev {rev['rev'] if rev else 'unchanged'}; "
            f"control revisions +{len(extension)} → {[i.get('cell') for i in items]}"]


def _assignment_plan(doc: dict) -> list:
    """[(node, cell, mode)] — 노드별 배정을 표시 순서로 편다. 옛 평면 `order` 의 후속이며,
    다른 노드의 리스트는 **동시에** 돈다는 사실이 이 자료구조의 요점이다."""
    v = validator()
    out: list = []
    for node in v.assignments_of(doc):
        for cell in v.assigned_cells(doc, node):
            out.append((node, cell, v.cell_mode(doc, node, cell)))
    return out


def resume_brief(camp_id: str | None = None) -> str:
    """인스턴스만 읽고 재개에 필요한 것을 한 화면으로 낸다. README 읽기 순서 **0번**.

    합격 기준(사용자): 이것만 읽고 **다음 셀에 착수**하고 **반증된 축을 재시도하지 않는다**.
    """
    camp = camp_id or active_campaign_id()
    base = campaigns_dir() / camp
    L: list[str] = []
    A = L.append
    # 첫 줄 = 수명 상태 한 줄(2026-10-17 · plan_26101719 D5). 위치를 고정하는 이유: 재개 에이전트가 이 줄 하나로
    # "물을 것이 있는가" 를 판정하고, 없으면 묻지 않는다 — 세션마다 같은 보류를 다시 묻는 결함을 여기서 끊는다.
    table = lifecycle_table()
    A(lifecycle_line(table))
    A(f"# resume-brief · campaigns/{camp}")
    _life = [(c, f) for c, f in table.items() if f["state"] != "closed"]
    if _life:
        A("")
        A("## 0. 캠페인 수명 (journey fold · 질의 규칙: 계기 a=파생 자식 종결 · d=무관 개설뿐)")
        for c, f in _life:
            bits = [f["state"]]
            if f.get("parent"):
                bits.append(f"parent={f['parent']}")
            if f.get("interrupted_cells"):
                bits.append(f"interrupted={','.join(f['interrupted_cells'])}")
            if f.get("sub_convergence"):
                bits.append(f"서브 {f['sub_convergence']}")
            if f["state"] in ("suspended", "resume_pending"):
                bits.append("답함" if f["answered"] else "미답")
            A(f"- `{c}` — " + " · ".join(bits))
        if any(f["state"] == "resume_pending" and not f["answered"] for _, f in _life):
            A("- 미결 질의는 **사람이 있는 세션에서 한 번** 묻는다(무인 리볼빙은 이 줄만 보이고 묻지 않는다 · D6). "
              "답: `--resume-campaign` · `--answer-query <id> --decision keep` · `--abandon`")
    if camp == BOOTSTRAP:
        A("")
        A("활성 캠페인이 **없다**(ACTIVE=_bootstrap). 여기는 캠페인 밖 릴레이 대기실이며 진행 상태를")
        A("들지 않는다 — campaigns writer 는 no-op 이고, 재개할 캠페인도 없다.")
        return "\n".join(L) + "\n"
    if not base.is_dir():
        A("")
        A(f"인스턴스 디렉터리가 없다: campaigns/{camp}")
        return "\n".join(L) + "\n"

    doc = _read_json(base / "campaign.yaml")
    doc = doc if isinstance(doc, dict) else {}
    revs = doc.get("revisions") if isinstance(doc.get("revisions"), list) else []
    cur = (revs[-1].get("values") if revs else None) or doc.get("control_variables") or {}
    A("")
    A("## 1. layer-1 선언 (통제변인)")
    if cur:
        for k in sorted(cur):
            A(f"- {k}: {cur[k]}")
    else:
        A("- (선언 없음 — 결손)")
    A(f"- 개정 수: {max(len(revs) - 1, 0)}"
      + (f" · 최신 사유: {revs[-1].get('reason')}" if len(revs) > 1 else ""))
    A(f"- plan_ref: {doc.get('plan_ref')}")

    plan = _assignment_plan(doc)
    cells_dir = base / "cells"
    found = sorted(p.name for p in cells_dir.iterdir()
                   if p.is_dir() and p.name != "_cell") if cells_dir.is_dir() else []
    known = {c for _, c, _ in plan}
    listed = plan + [(None, c, None) for c in found if c not in known]
    A("")
    A("## 2. 셀 진행표 (노드별 배정 순 — **다른 노드의 리스트는 동시에 돈다**)")
    pending, refuted, reentry_pending = [], [], []
    for node_of, cell, mode in listed:
        st = _read_json(cells_dir / cell / "cell.status.json")
        st = st if isinstance(st, dict) else {}
        outcome = st.get("cell_outcome") or ("(상태파일 없음)" if (cells_dir / cell).is_dir()
                                             else "(미개설)")
        tps = ((st.get("measurement") or {}).get("decode_tps_conc1")
               if isinstance(st.get("measurement"), dict) else None)
        cite = st.get("axis_citation")
        extra = []
        if tps is not None:
            extra.append(f"decode {tps} t/s")
        if st.get("void_reason"):
            extra.append(f"void: {_short(st['void_reason'], 110)}")
        # lite ② 재발동(2026-09-29 · plan_26092923_58_27) — 차감 수와 제안 상태를 셀 줄에 싣는다(잔여 cap 은 explorer 가 파생).
        _rc = st.get("reconciliation") if isinstance(st.get("reconciliation"), dict) else {}
        _re = st.get("reentry") if isinstance(st.get("reentry"), dict) else {}
        if _rc.get("spent"):
            extra.append(f"lite ② · cap 차감 {_rc['spent']} · 재발동 {_re.get('status') or '기록 없음'}")
        if _re.get("status") == "proposed":
            reentry_pending.append((cell, _rc.get("spent"), _re.get("proposed_utc")))
        tag = f"[{node_of} · {mode}]" if node_of else "[미배정]"
        A(f"- `{cell}` {tag} — {outcome}" + (f"  ({' · '.join(extra)})" if extra else ""))
        # 상태 파일이 아예 없는 셀도 **남은 작업**이다 — "돌지 않았다"와 "종결했다"를 같은 값으로
        # 접으면 전 셀 미개설인 새 캠페인이 '완주' 로 보인다(부재 ≠ 통과).
        if outcome in CELL_OUTCOMES_PENDING + ("(상태파일 없음)", "(미개설)"):
            pending.append(cell)
        if outcome in CELL_OUTCOMES_REFUTED:
            refuted.append((cell, cite, st.get("void_reason")))
    if not listed:
        A("- (배정된 셀 없음)")
    if reentry_pending:
        A("")
        A("### 재발동 대기(사람 승인) — lite ② 실사용 불가")
        for cell, spent, when in reentry_pending:
            A(f"- `{cell}` — cap 차감 누계 {spent} · 제안 {when or '시각 미상'} · 경로: "
              + " / ".join(REENTRY_ROUTES.values()) + " · 결정은 `--reentry-decide` 로 기록(자동 실행 ✗)")

    A("")
    A("## 3. 노드별 phase 진행표")
    phases_dir = base / "phases"
    nodes = [n.get("node_id") for n in (doc.get("nodes") or [])
             if isinstance(n, dict) and isinstance(n.get("node_id"), str)
             and FILL not in n.get("node_id")]
    seen = sorted(p.name for p in phases_dir.iterdir()
                  if p.is_dir() and p.name != "_node") if phases_dir.is_dir() else []
    for n in seen:
        if n not in nodes:
            nodes.append(n)
    for node in nodes or ["(선언 노드 없음)"]:
        row = []
        for ph in PHASE_NAMES:
            st = _read_json(phases_dir / node / f"{ph}.status.json")
            st = st if isinstance(st, dict) else None
            if st is None:
                row.append(f"{ph}=—")
            else:
                mark = "✓" if (st.get("proof") or {}).get("ok") else "·"
                row.append(f"{ph}={st.get('state')}{mark}")
        missing = "  ⚠ phases/ 디렉터리 없음" if not (phases_dir / node).is_dir() else ""
        A(f"- {node}: " + " ".join(row) + missing)

    jour = read_journey(base)
    A("")
    A("## 4. 여정 (마지막 3줄)")
    for e in jour[-3:]:
        A(f"- [{e.get('utc')}] {e.get('cell_id')} → {_short(e.get('next_intent'))}"
          + (f"   (축: {_short(e.get('axis_citation'), 90)})" if e.get("axis_citation") else ""))
    if not jour:
        A("- (여정 줄 없음 — **결손**. 이 체인에서 유일하게 복원 불가능한 정보다)")

    A("")
    A("## 5. 다음에 할 일")
    last_intent = jour[-1].get("next_intent") if jour else None
    A(f"- 다음 pending 셀: {pending[0] if pending else '없음(전 셀 종결)'}"
      + (f"   · 이후 {len(pending) - 1}건" if len(pending) > 1 else ""))
    A(f"- 마지막 여정이 지목한 다음 의도: {last_intent or '(없음)'}")
    A("")
    A("## 6. 반증된 축 — **재시도 금지**")
    if refuted:
        for cell, cite, why in refuted:
            A(f"- `{cell}`: {_short(why) if why else '(사유 미기재)'}"
              + (f"   [축: {_short(cite, 110)}]" if cite else ""))
    else:
        A("- (없음)")

    ep = _read_json(base / "evidence_pointers.json")
    ptrs = (ep or {}).get("pointers") if isinstance(ep, dict) else None
    kinds = {str(p.get("kind")) for p in (ptrs or []) if isinstance(p, dict)}
    A("")
    A("## 7. 증거")
    A(f"- 포인터 {len(ptrs or [])}건 · 종류 {sorted(kinds) if kinds else '없음'}")
    A(f"- relay_summary: {'있음' if 'relay_summary' in kinds else '**없음(purge 선행조건 미충족)**'}")
    if isinstance(ep, dict) and ep.get("frozen_utc"):
        A(f"- 스냅샷 동결: {ep['frozen_utc']} (이후 --evidence-add 는 거부된다)")
    return "\n".join(L) + "\n"


# ─────────────────────────────────────────────────────────────────────────────
# 파생 선언 · 관측면 (2026-09-08 신설 · plan_26090813 §4.2)
#
# 왜: 싱글 토폴로지의 sub 는 A2A 원격 에이전트이고, **지시서만으로 착수**할 수 있어야 한다
# (사용자 정정 2026-09-08: 메인의 선행 산출물을 받아야 움직이는 것은 자율이 아니라 종속이다).
# 그래서 메인은 빌딩블럭이 아니라 **자기 몫만 담은 선언**을 보내고, 서브는 그것으로 자기
# campaigns/ 인스턴스를 연다. 그리고 진행은 서브가 쓰고 메인이 **문서로** 가져온다 —
# 종전에는 그 자리가 비어 있어서 메인이 ssh 로 서브를 32회 직접 관측했다(헌법 노드제어 ①).

SLICE_AUTHOR = "main-derived"


def _sub_self_publishes(topology) -> bool:
    """서브 자체 발행 판정 — 소유자는 `node_role_contract.sub_self_publishes`(파생). 계약기는 **메인에만** 있으므로
    (서브에는 배달되지 않는다) 여기서 지연 적재한다 — emit_slice 는 메인 오케스트레이션이라 그 자리에 계약기가 있다.
    topology 를 모르면 추측하지 않는다(WriterRefusal)."""
    import importlib.util
    mod_path = Path(__file__).resolve().parent / "node_role_contract.py"
    if not mod_path.is_file():
        raise WriterRefusal(f"계약기를 찾지 못했다: {_rel(mod_path)} — 파생 선언은 메인에서만 자른다")
    spec = importlib.util.spec_from_file_location("_campaign_node_role_contract", mod_path)
    if spec is None or spec.loader is None:
        raise WriterRefusal("계약기 적재 실패")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    if topology not in mod.TOPOLOGIES:
        raise WriterRefusal(f"노드 topology {topology!r} 를 모른다 — 그 노드 몫 hint 승인을 운반할지 판정할 수 없다"
                            f"(어휘 {list(mod.TOPOLOGIES)} · 추측 ✗)")
    return bool(mod.sub_self_publishes(topology)["value"])


def emit_slice(base: Path, node: str, *, utc: str) -> dict:
    """`assignments[node]` 만 남긴 파생 선언. 파생 결정은 **메인 오케스트레이션**이다(D12).

    공유하는 것: id·plan_ref·matrix·budgets·control_variables·revisions(echo)·topology_sections.
    자르는 것: nodes(그 노드 하나) · assignments(그 노드 몫) · hint_targets(그 노드 몫만 · 아래).

    hint_targets(2026-10-11 · plan_26101104 Step 3): 그 노드가 **자체 발행하는 서브**(`node_role_contract.sub_self_publishes`
    — tool_plane 에 hint-publisher 가 있는가 · 지금은 single)면 메인 선언의 `hint_targets` 중 **그 노드 몫 항목을 승인째**
    싣는다. 다른 노드(메인 포함) 몫은 자른다. 자체 발행하지 않는 노드(멀티 Ray 워커)·메인 노드는 빈 목록이다. 판정은
    파생이다 — 계약기가 바뀌면 운반도 따라간다. 노드의 topology 를 모르는데 그 노드 몫 승인이 있으면 추측하지 않고 거부한다.
    """
    decl = read_declaration(base)
    if not decl:
        raise WriterRefusal(f"선언을 읽지 못했다: {_rel(base / 'campaign.yaml')}")
    v = validator()
    nodes = [n for n in (decl.get("nodes") or []) if isinstance(n, dict) and n.get("node_id") == node]
    if not nodes:
        raise WriterRefusal(f"nodes[] 에 {node!r} 가 없다 — 없는 노드에 배정을 자를 수 없다")
    items = v.assignment_items(decl, node)
    if not items:
        raise WriterRefusal(f"assignments[{node!r}] 가 비었다 — 보낼 배정이 없는 지시서는 지시서가 아니다")
    mains = [n.get("node_id") for n in (decl.get("nodes") or [])
             if isinstance(n, dict) and n.get("role") == "main"]
    own_targets = [t for t in (decl.get("hint_targets") or []) if isinstance(t, dict) and t.get("node_id") == node]
    carried: list = []
    if own_targets and nodes[0].get("role") == "sub":
        why = v.hint_target_reasons(decl)
        if why:
            raise WriterRefusal("메인 선언의 hint_targets 에 결함이 있다 — 결함 있는 승인을 운반하지 않는다:\n"
                                + "\n".join(f"  - {b}" for b in why))
        if _sub_self_publishes(nodes[0].get("topology")):
            carried = [json.loads(json.dumps(t, ensure_ascii=False)) for t in own_targets]   # 승인째 깊은 복사
    out = {
        "schema_version": 1,
        "id": decl.get("id"),
        "plan_ref": decl.get("plan_ref"),
        "declared_utc": decl.get("declared_utc"),
        "nodes": nodes,
        "matrix": decl.get("matrix"),
        "assignments": {node: items},
        "budgets": decl.get("budgets"),
        "control_variables": decl.get("control_variables"),
        "revisions": decl.get("revisions") or [],
        "hint_targets": carried,
        "topology_sections": decl.get("topology_sections") or {},
        "self_role": nodes[0].get("role"),
        "authored_by": SLICE_AUTHOR,
        "derived_from": mains[0] if mains else None,
        "derived_utc": utc,
        # 파생 계보(2026-10-17 · plan_26101719 D2) — 서브가 같은 id 의 인스턴스를 열 때 부모를 잇는다. `_` 접두라
        # scaffold 가 선언에는 싣지 않는다(선언의 시민이 아니라 개설 인자다).
        "_lifecycle_parent": fold_lifecycle(base).get("parent"),
        # 재배정 메타(K8) — 서브 `--slice-update` 가 사유·승인을 그대로 전사한다. 개설 전이면 None.
        "_assignment_revision": next((({k: r.get(k) for k in ("rev", "reason", "evidence", "approved_by",
                                                               "declared_utc")})
                                      for r in reversed(decl.get("assignment_revisions") or [])
                                      if isinstance(r, dict) and r.get("node") == node), None),
    }
    return out


def campaign_brief(base: Path, *, node: str, utc: str) -> dict:
    """**선언된 관측면** — 서브가 자기 진행을 기계판독으로 내는 한 파일.

    메인은 이 파일 외에 서브를 읽지 않는다(§2.7.7 `sub.campaign.brief`). `last_utc` 를 함께 내는
    이유: 감독 스텝이 "전진했는가" 를 phase 변화만으로 물으면 긴 벤치 중 정체로 오판한다(R2).
    """
    decl = read_declaration(base)
    v = validator()
    cells_dir = base / "cells"
    assigned = v.assigned_cells(decl, node) or v.assigned_cells(decl)
    # 디스크에 있는 셀 중 **다른 노드에 배정된 것**은 이 노드의 브리핑에 싣지 않는다
    # (2026-09-08 라이브 교정): 실으면 메인 브리핑이 서브 셀까지 `pending` 으로 세고, 그 목록을
    # 읽는 감독 스텝이 "메인이 아직 4셀 남았다" 고 오판한다. 배정되지 않은 셀은 주인이 없으므로
    # 남긴다 — 그건 결손이고 보여야 한다.
    others = {c for nid in v.assignments_of(decl) if nid != node
              for c in v.assigned_cells(decl, nid)}
    on_disk = sorted(p.name for p in cells_dir.iterdir()
                     if p.is_dir() and p.name != "_cell") if cells_dir.is_dir() else []
    listed = assigned + [c for c in on_disk if c not in assigned and c not in others]
    stamps: list[str] = []
    cells = []
    for cell in listed:
        st = _read_json(cells_dir / cell / "cell.status.json")
        st = st if isinstance(st, dict) else {}
        meas = st.get("measurement") if isinstance(st.get("measurement"), dict) else {}
        cells.append({
            "cell_id": cell,
            "mode": v.cell_mode(decl, node, cell),
            "cell_outcome": st.get("cell_outcome"),
            "decode_tps_conc1": meas.get("decode_tps_conc1"),
            "measurement_gap": meas.get("gap"),
            "axis_citation": st.get("axis_citation"),
            "void_reason": st.get("void_reason"),
        })
    phases = {}
    for ph in PHASE_NAMES:
        st = _read_json(base / "phases" / node / f"{ph}.status.json")
        if not isinstance(st, dict):
            phases[ph] = None
            continue
        phases[ph] = {"state": st.get("state"),
                      "proof_ok": bool((st.get("proof") or {}).get("ok")),
                      "proof_source": (st.get("proof") or {}).get("source"),
                      "first_started_utc": st.get("first_started_utc"),
                      "started_utc": st.get("started_utc"),
                      "ended_utc": st.get("ended_utc")}
        stamps += [x for x in (st.get("started_utc"), st.get("ended_utc")) if isinstance(x, str)]
    jour = read_journey(base)
    stamps += [j.get("utc") for j in jour if isinstance(j.get("utc"), str)]
    ep = _read_json(base / "evidence_pointers.json")
    ptrs = (ep or {}).get("pointers") if isinstance(ep, dict) else None
    claims = [dict(ptr) for ptr in (ptrs or []) if isinstance(ptr, dict)
              and ptr.get("node_id") == node]
    return {
        "schema_version": 1,
        "generated_utc": utc,
        "campaign_id": base.name,
        "node_id": node,
        "self_role": decl.get("self_role") or next(
            (n.get("role") for n in (decl.get("nodes") or [])
             if isinstance(n, dict) and n.get("node_id") == node), None),
        "authored_by": node,
        "plan_ref": decl.get("plan_ref"),
        "cells": cells,
        "pending_cells": [c["cell_id"] for c in cells
                          if c["cell_outcome"] is None
                          or c["cell_outcome"] in CELL_OUTCOMES_PENDING],
        "phases": phases,
        "journey_tail": jour[-3:],
        # 서브 자기 인스턴스의 수명 상태(2026-10-17 · plan_26101719 D9) — 메인 감독이 "서브가 실제로 보류했는가" 를
        # 이 관측면 하나로 읽는다(ssh 직접 관측 ✗). 판정은 같은 fold 다.
        "lifecycle": {k: v for k, v in fold_lifecycle(base).items()
                      if k in ("state", "close_reason", "parent", "last_transition_event", "interrupted_cells")},
        "evidence_pointer_count": len(ptrs or []),
        # 이 brief가 주장하는 node-scoped evidence membership. 원천은 campaign instance의
        # evidence_pointers.json이고, importer는 filename/시각 추측 대신 이 선언만 소비한다.
        "evidence_pointers": claims,
        # 전진 신호. phase 만 보면 긴 벤치가 정체로 보인다(R2) — 여정·시각도 전진으로 센다.
        "last_utc": max(stamps) if stamps else None,
    }


# 회수 미러의 파일 이름 → 증거 종류. 이름 규약의 정본은 `doc_naming.py` 이고 여기는 **소비자**다
# (접두어 목록은 docs.md §명명 SSOT 가 정한 것이고, 새 종류가 생기면 여기도 같이 는다).
_EVIDENCE_PREFIX = (
    ("benchmark/benchmark_", ".yaml", "certificate"),
    ("benchmark/bench_report_", ".md", "bench_report"),
    ("benchmark/sweep_map_", ".md", "sweep_map"),
    ("testlog/testlog_", ".md", "testlog"),
    ("devlog/devlog_", ".md", "devlog"),
)


def import_sub_mirror(base: Path, mirror: Path, *, utc: str,
                      unfreeze: bool = False) -> "tuple[list[str], list[str]]":
    """Import only the current campaign's declared brief claims from a shared docs mirror."""
    wrote: list[str] = []
    warnings: list[str] = []
    discovered = sorted(mirror.glob("logs/*/campaign_brief.json"))
    if not discovered:
        raise WriterRefusal(
            f"회수 미러에 브리핑이 없다: {_rel(mirror)}/logs/*/campaign_brief.json — 서브가 "
            "`campaign_init --write-brief` 를 돌지 않았거나 회수 범위가 docs/logs 를 뺐다.")
    briefs: list[tuple[Path, dict]] = []
    for bpath in discovered:
        doc = _read_json(bpath)
        if not isinstance(doc, dict):
            raise WriterRefusal(f"브리핑 파손: {_rel(bpath)}")
        if doc.get("campaign_id") != base.name:
            continue                    # standing mirror's historical campaign, not an error
        node = doc.get("node_id")
        if not isinstance(node, str) or not node:
            raise WriterRefusal(f"브리핑에 node_id 가 없다: {_rel(bpath)}")
        briefs.append((bpath, doc))
    if not briefs:
        raise WriterRefusal(f"현재 캠페인({base.name})의 브리핑이 회수 미러에 없다 — "
                            "다른 캠페인의 역사로 현재 진행표를 만들지 않는다")
    # Validate every claim in every current brief before the first write. Previously an invalid
    # final relay pointer raised only after phases/cells/journey and earlier evidence were changed.
    claim_paths: dict[tuple[str, int], Path] = {}
    for bpath, doc in briefs:
        node = doc["node_id"]
        _sub_evidence_binding(base, node, None)
        phases = doc.get("phases") or {}
        cells = doc.get("cells") or []
        journey = doc.get("journey_tail") or []
        if not isinstance(phases, dict) or not isinstance(cells, list) or not isinstance(journey, list):
            raise WriterRefusal(f"브리핑의 phases/cells/journey_tail 형식이 무효다: {_rel(bpath)}")
        for phase, st in phases.items():
            if not isinstance(st, dict) or phase not in PHASE_NAMES:
                continue
            if (st.get("state") or "pending") not in PHASE_STATES:
                raise WriterRefusal(f"브리핑 phase state가 무효다: {phase}={st.get('state')!r}")
            _no_fill(source=st.get("proof_source") or "")
        for cell in cells:
            if isinstance(cell, dict) and cell.get("cell_id") and cell.get("cell_outcome") in CELL_OUTCOMES:
                _sub_evidence_binding(base, node, cell["cell_id"])
        for entry in journey:
            if isinstance(entry, dict) and not _is_lifecycle(entry):
                if entry.get("node_id") not in (None, node):
                    raise WriterRefusal("브리핑 journey의 node_id가 brief node와 다르다")
                _sub_evidence_binding(base, node, entry.get("cell_id"))
        claims = doc.get("evidence_pointers")
        if claims is None:
            continue
        if not isinstance(claims, list):
            raise WriterRefusal(f"브리핑 evidence_pointers 가 목록이 아니다: {_rel(bpath)}")
        for i, claim in enumerate(claims):
            if not isinstance(claim, dict):
                raise WriterRefusal(f"브리핑 evidence claim 이 객체가 아니다: {_rel(bpath)}")
            kind, path, claim_node = claim.get("kind"), claim.get("path"), claim.get("node_id")
            if claim_node != node:
                raise WriterRefusal(f"브리핑 evidence claim node_id({claim_node!r})가 brief node({node!r})와 다르다")
            _sub_evidence_binding(base, node, claim.get("cell_id"))
            local = _sub_evidence_path(kind, path, mirror)
            try:
                local.relative_to(REPO_ROOT)
            except ValueError as exc:
                raise WriterRefusal(f"회수 evidence mirror는 저장소 안이어야 한다: {_rel(mirror)}") from exc
            claim_paths[(str(bpath), i)] = local
    for bpath, doc in briefs:
        node = doc["node_id"]
        for phase, st in (doc.get("phases") or {}).items():
            if not isinstance(st, dict) or phase not in PHASE_NAMES:
                continue
            wrote.append(_rel(writer_set_phase(
                base, node=node, phase=phase, state=st.get("state") or "pending",
                predicate=f"서브 회수(brief {doc.get('generated_utc')})",
                ok=bool(st.get("proof_ok")), source=st.get("proof_source") or f"{_rel(bpath)}#phases.{phase}",
                cell_id=None, started_utc=st.get("first_started_utc") or st.get("started_utc"),
                ended_utc=st.get("ended_utc"), authored_by=node)))
        for cell in (doc.get("cells") or []):
            if not isinstance(cell, dict) or not cell.get("cell_id") or cell.get("cell_outcome") not in CELL_OUTCOMES:
                continue
            tps = cell.get("decode_tps_conc1")
            cpath, jpath = writer_set_cell(
                base, cell=cell["cell_id"], outcome=cell["cell_outcome"], node=node,
                version=None, model=None, decode_tps=str(tps) if isinstance(tps, (int, float)) else None,
                measurement_source=f"{_rel(bpath)}#cells[{cell['cell_id']}]" if isinstance(tps, (int, float)) else None,
                void_reason=cell.get("void_reason"),
                void_reason_source=f"{_rel(bpath)}#cells[{cell['cell_id']}].void_reason" if cell.get("void_reason") else None,
                axis_citation=cell.get("axis_citation"), next_intent=None, utc=utc)
            wrote.append(_rel(cpath))
            if jpath is not None:
                wrote.append(_rel(jpath))
        def _jkey(entry: dict) -> str:
            return json.dumps({k: v for k, v in entry.items() if k not in ("imported_from", "imported_utc")},
                              ensure_ascii=False, sort_keys=True)
        have = {_jkey(entry) for entry in read_journey(base) if isinstance(entry, dict)}
        for entry in (doc.get("journey_tail") or []):
            # 수명 이벤트는 **편입하지 않는다**(2026-10-17 · plan_26101719 D1). 메인 인스턴스의 수명은 메인
            # journey 가 단일 권위이고, 서브의 보류·재개는 echo(sub_control_echo)로만 들어온다 — 서브 줄을 그대로
            # 섞으면 메인의 fold 가 서브의 전이를 자기 것으로 접는다.
            if _is_lifecycle(entry):
                continue
            if isinstance(entry, dict) and _jkey(entry) not in have:
                have.add(_jkey(entry))
                wrote.append(_rel(writer_append_journey(
                    base, dict(entry, imported_from=_rel(bpath), imported_utc=utc))))
        claims = doc.get("evidence_pointers")
        if claims is None:
            warnings.append(f"브리핑에 scoped evidence_pointers 가 없다: {_rel(bpath)} — 진행표만 편입")
            continue
        for i, claim in enumerate(claims):
            kind = claim["kind"]
            local = claim_paths[(str(bpath), i)]
            if not local.is_file():
                warnings.append(f"증거 미등재 {_rel(local)} (brief가 선언했으나 미러에 없다)")
                continue
            try:
                wrote.append(_rel(writer_add_evidence(
                    base, kind=kind, path_rel=str(local.relative_to(REPO_ROOT)),
                    cell_id=claim.get("cell_id"), node=node, unfreeze=unfreeze)))
            except WriterRefusal as exc:
                warnings.append(f"증거 미등재 {_rel(local)} (kind={kind}) — {exc}")
    return wrote, warnings


def _sweep_sources(base: Path) -> list[Path]:
    """측정값이 살아 있는 자리 전부. 인스턴스의 스윕 기록이 1순위이고, docs 평면의 sweep map 은
    인스턴스가 결손일 때의 **사후 출처**다(둘 다 같은 `cell_key`·`concurrency_vector` 모양)."""
    out = sorted(base.glob("sweeps/*.json"))
    for d in (REPO_ROOT / "docs" / "benchmark",
              REPO_ROOT / "sync_staging" / "sub_docs" / "benchmark"):
        if d.is_dir():
            out += sorted(d.glob("sweep_map_*.json"))
    return out


def backfill_from_docs(base: Path, *, utc: str | None = None) -> "tuple[list[str], list[str]]":
    """결손 셀의 측정·노드를 사후 수리한다(2026-09-08 · plan_26090813 §4.4 · 사용자 결정 D17).

    왜 차단이 아니라 수리인가: 결손을 게이트로 막으면 캠페인이 그 자리에서 선다(사용자 경계 —
    "너무 결정론적이면 시스템이 무한 hang 에 빠진다"). 대신 **hint 발행 직전에** 값이 실제로
    있는 자리(스윕 기록·sweep map)를 훑어 채우고, 그래도 없는 것은 결손으로 남긴 채 이름을 부른다.
    """
    repaired: list[str] = []
    gaps: list[str] = []
    decl = read_declaration(base)
    v = validator()
    records: dict = {}
    for src in _sweep_sources(base):
        doc = _read_json(src)
        if not isinstance(doc, dict):
            continue
        for rec in (doc.get("cells") or []):
            if not isinstance(rec, dict):
                continue
            key = rec.get("cell_key")
            vec = rec.get("concurrency_vector")
            val = vec.get("1") if isinstance(vec, dict) else None
            if isinstance(key, str) and isinstance(val, (int, float)) and not isinstance(val, bool):
                records.setdefault(key, (float(val), rec.get("cell_outcome"), _rel(src)))
    cells_dir = base / "cells"
    known = set(v.assigned_cells(decl))
    on_disk = {p.name for p in cells_dir.iterdir()
               if p.is_dir() and p.name != "_cell"} if cells_dir.is_dir() else set()
    for cell in sorted(known | on_disk):
        st = _read_json(cells_dir / cell / "cell.status.json")
        st = st if isinstance(st, dict) else {}
        meas = st.get("measurement") if isinstance(st.get("measurement"), dict) else {}
        need_tps = meas.get("decode_tps_conc1") is None
        need_node = not st.get("node_id")
        if not (need_tps or need_node or not st):
            continue
        rec = records.get(cell)
        if need_tps and rec is None:
            gaps.append(f"{cell}: 측정값이 어느 자리에도 없다(스윕 기록·sweep map 전수 조회)")
            if st and not need_node:
                continue
        tps, sweep_outcome, src = rec if rec else (None, None, None)
        outcome = st.get("cell_outcome") or sweep_outcome or "pending"
        if outcome not in CELL_OUTCOMES:
            outcome = "pending"
        cpath, _ = writer_set_cell(
            base, cell=cell, outcome=outcome, node=st.get("node_id"),
            version=st.get("version"), model=st.get("model"),
            decode_tps=(str(tps) if need_tps and tps is not None else None),
            measurement_source=((f"backfill:{src}#cells[cell_key={cell}].concurrency_vector.1")
                                if need_tps and tps is not None else None),
            void_reason=st.get("void_reason"), void_reason_source=st.get("void_reason_source"),
            axis_citation=st.get("axis_citation"), next_intent=None, utc=utc)
        after = _read_json(cpath) or {}
        if not after.get("node_id"):
            gaps.append(f"{cell}: 노드를 배정에서 파생하지 못했다(assignments 에 없는 셀)")
        repaired.append(_rel(cpath))
    return repaired, gaps


# ─────────────────────────────────────────────────────────────────────────────
# 그라운딩 안전불변식 (2026-09-08 신설 · policy:LIBRARY_GROUNDING_FAIL_CLOSED · plan_26090813 §4.5)
#
# 왜 안전불변식 강도인가(사용자 결정 D4): 도서관은 구축돼 있었고 **활용이 0** 이었다 —
# camp-26090721 세션에서 wiki-desk 호출 0회, 서가는 09-03 이후 145건 미입고, 절차는 권고문뿐이라
# 실행자도 게이트도 없었다. 사용자는 이것을 **사고**로 판정하고, 벤치 스킬의 "외부검색을 실제로
# 수행했는가" 불변식과 같은 계통으로 올리라고 지시했다. 그 불변식의 모양을 그대로 따른다:
#   진입 백스톱(fail-closed) + 자기 선언 출처 + 사람 백스톱(해소 실패는 기재 후 진행).
#
# ★ 누락은 기계가 fail-closed 로 잡고 거짓은 사람이 리뷰한다(헌법 불변식 B). 그래서 "사서가 못
#   찾았다"는 차단이 아니라 **기재**다 — 차단하면 도서관에 없는 새 주제를 영영 못 돈다.

GROUNDING_DIR = "grounding"


# 사서 CLI(TS · plan_26101912). 같은 명령이 메인(접근자 local)에서는 자기 서고를, 서브(접근자 ssh)에서는 메인 서고를
# 게이트웨이로 부른다 — 경로만 다르다. 반출 상한(질의 결과 수)은 이 그라운딩 기록의 소유자가 정한다.
WIKI_CLI = ".claude/skills/wiki-desk/src/cli.ts"
GROUNDING_TOP = 6
_EXIT_NOT_INGESTED = 4       # wiki-desk remote.ts EXIT_NOT_INGESTED — 서고 미도달(발행·질의를 막지 않는다)
_EXIT_GATEWAY_REFUSED = 126  # wiki-desk gateway.ts — 서고가 요청을 **거절**했다(공백과 다른 사실)


def _wiki(root: Path, *args: str):
    import subprocess
    return subprocess.run(["node", str(root / WIKI_CLI), *args, "--root", str(root)],
                          capture_output=True, text=True, cwd=str(root), check=False)


def warm_start_library(repo_root: Path | None = None) -> str:
    """질의 전에 서가에 입고한다(C4). 정지한 서가에 물으면 **없다는 답이 거짓**이 된다.

    2026-09-08 실측: 서가가 09-03 에 멈춰 있어 캠페인 문서 147건이 미입고였고, 그 상태의 사서는
    "관련 자료 없음" 이라고 답했을 것이다 — 자료는 실재했다. 발행기·publish 위상·질의 진입이
    모두 이 함수를 부른다(호출부 N · 실행자를 두지 않으면 그 절차는 권고문이다).
    서고 미도달(exit 4)은 not-yet-ingested 로 **기재**한다 — 다음 입고가 따라잡는다.
    """
    root = REPO_ROOT if repo_root is None else Path(repo_root)
    if not (root / WIKI_CLI).is_file():
        return f"입고 건너뜀 — 사서 도구 부재({_rel(root / WIKI_CLI)})"
    cp = _wiki(root, "sync", "--apply")
    if cp.returncode not in (0, _EXIT_NOT_INGESTED):
        return f"입고 실패(rc={cp.returncode}): {(cp.stderr or '').strip()[-200:]}"
    try:
        out = json.loads(cp.stdout) or {}
    except ValueError:
        return "입고 완료(출력 판독 불가)"
    if out.get("ingest_status") == "not-yet-ingested":
        return f"not-yet-ingested — 서고 미도달(stage={out.get('stage')}) · 다음 입고가 따라잡는다"
    return (f"입고 완료 — created={len(out.get('created') or [])} updated={len(out.get('updated') or [])} "
            f"archived={len(out.get('archived') or [])}")


def query_library(terms: list, repo_root: Path | None = None, top: int = GROUNDING_TOP) -> dict:
    """사서 질의 → 반출(질의 결과 포인터). 해소 실패는 **정직한 공백**(`unresolved` + 사유)이고,
    게이트웨이가 거절하면 `refused` 다. 없는 근거를 지어내지 않는다 — 결과 경로·근거 구간만 옮긴다."""
    root = REPO_ROOT if repo_root is None else Path(repo_root)
    if not (root / WIKI_CLI).is_file():
        return {"resolution": {"status": "unresolved", "librarian": "wiki-desk",
                               "reason": f"사서 도구 부재({_rel(root / WIKI_CLI)})"}, "references": []}
    argv = ["query", "--limit", str(top)]
    for t in terms:
        argv += ["--terms", str(t)]
    cp = _wiki(root, *argv)
    if cp.returncode == _EXIT_GATEWAY_REFUSED:
        return {"resolution": {"status": "refused", "librarian": "wiki-desk",
                               "reason": (cp.stderr or "").strip()[-300:]}, "references": []}
    try:
        out = json.loads(cp.stdout) if cp.returncode == 0 else None
    except ValueError:
        out = None
    if out is None:
        return {"resolution": {"status": "unresolved", "librarian": "wiki-desk",
                               "reason": f"사서 질의 실패(rc={cp.returncode}): {(cp.stderr or '').strip()[-200:]}"},
                "references": []}
    refs = [{"ref_id": f"R{i}", "path": r.get("path"), "score": r.get("score"),
             "spans": [sp.get("ref") for sp in (r.get("spans") or [])]}
            for i, r in enumerate(out.get("results") or [], start=1)]
    return {"resolution": {"status": "resolved" if refs else "unresolved", "librarian": "wiki-desk",
                           "reason": None if refs else "질의 결과 0건(정직한 공백)",
                           "unmatched_tokens": out.get("unmatched_tokens"), "stale": out.get("stale")},
            "references": refs}


def grounding_terms(decl: dict) -> list[str]:
    """통제변인에서 질의어를 **파생**한다. 손으로 적으면 선언과 갈라지고, 갈라진 질의는
    캠페인이 실제로 도는 것과 다른 것을 찾는다."""
    cv = decl.get("control_variables") or {}
    out: list[str] = []
    for key in sorted(cv):
        if key.startswith("_"):
            continue
        val = cv[key]
        if isinstance(val, (str, int, float)) and not isinstance(val, bool):
            text = str(val).strip()
            if text and FILL not in text and len(text) <= 80:
                out.append(text)
    return out


def latest_grounding(base: Path) -> "tuple[Path, dict] | None":
    """가장 최근 그라운딩 기록. 파일명이 UTC 라 사전순 = 시간순이다."""
    d = base / GROUNDING_DIR
    for path in sorted(d.glob("*.json"), reverse=True) if d.is_dir() else []:
        doc = _read_json(path)
        if isinstance(doc, dict):
            return path, doc
    return None


def grounding_reasons(base: Path) -> list[str]:
    """그라운딩이 성립하지 않는 **사유 목록**(빈 리스트 = 통과). C2 백스톱이 이것을 읽는다.

    통과 조건: 기록이 있고, 사서 판정이 `resolved` 또는 `unresolved`(정직한 공백 · 기재 후 진행).
    `refused` 만 차단이다 — 도서관이 답을 **거절**한 것은 공백과 다른 사실이다(D5).
    """
    found = latest_grounding(base)
    if found is None:
        return [f"그라운딩 기록이 없다: {_rel(base / GROUNDING_DIR)}/ — 캠페인 착수·셀 축 변경 시 "
                f"통제변인 파생 질의로 사서를 부른 기록이 있어야 한다"
                f"(policy:LIBRARY_GROUNDING_FAIL_CLOSED C1). "
                f"`campaign_init.py --ground --utc <t>` 로 물어라."]
    path, doc = found
    status = ((doc.get("export") or {}).get("resolution") or {}).get("status")
    if status == "refused":
        return [f"사서가 질의를 **거절**했다({_rel(path)}) — 공백과 거절은 다른 사실이고 "
                f"거절은 차단이다(D5)"]
    if status not in ("resolved", "unresolved"):
        return [f"그라운딩 기록의 판정이 알 수 없는 값이다({_rel(path)}): {status!r}"]
    return []


def ground_campaign(base: Path, *, utc: str, topology: str, terms: list | None = None,
                    warm_start: bool = True) -> "tuple[Path, dict]":
    """통제변인 파생 질의로 사서를 부르고 3메시지 모양으로 기록한다(C1).

    request(무엇을 찾는가) · export(사서 회신) · attestation(그 회신을 어떻게 썼는가). 메인과 서브가
    **같은 함수**로 기록한다 — 서브는 접근자(ssh)로 메인 서고에 묻고 경로만 다르다(plan_26101912 D2).
    """
    node_id = _self_node_id()
    decl = read_declaration(base)
    terms = terms or grounding_terms(decl)
    if not terms:
        raise WriterRefusal(
            "통제변인에서 질의어를 파생하지 못했다 — 선언이 아직 빈칸이면 그라운딩은 "
            "선언을 채운 뒤에 한다(진입 백스톱이 그때까지 서빙을 막는다).")
    note = warm_start_library() if warm_start else "입고 생략(호출부 지시)"
    request = {"schema_version": 1, "kind": "library.resolution.request",
               "exchange_id": f"{base.name}-{utc}", "node_id": node_id, "topology": topology,
               "query": {"terms": terms},
               "purpose": "캠페인 착수 그라운딩 — 이 통제변인으로 참고할 내부 자료가 있는가"}
    export = query_library(terms)
    status = ((export.get("resolution") or {}).get("status"))
    doc = {
        "schema_version": 1, "campaign_id": base.name, "asked_utc": utc,
        "library_intake": note,
        "request": request, "export": export,
        "attestation": {
            "kind": "library.resolution.attestation", "exchange_id": request["exchange_id"],
            "node_id": node_id, "status": status,
            "cited_refs": [r.get("ref_id") for r in (export.get("references") or [])],
            # 해소 실패는 **정직한 공백**이다 — 기재하고 진행한다(차단 ✗ · D5·D17).
            "grounding_gap": (None if status == "resolved"
                              else {"status": status, "asked_utc": utc,
                                    "reason": ((export.get("resolution") or {}).get("reason"))}),
        },
    }
    path = base / GROUNDING_DIR / f"{utc.replace(':', '').replace('-', '')}.json"
    _write_json(path, doc)
    return path, doc


def brief_path(node: str, repo_root: str | Path | None = None) -> Path:
    """기계판독 데이터 평면(`docs/logs/`). 산문 명명 SSOT 의 명시 예외이며 fetch_sub_docs 가
    이미 미러하는 경로다 — 새 회수 경로를 만들지 않는다."""
    root = REPO_ROOT if repo_root is None else Path(repo_root)
    return root / "docs" / "logs" / node / "campaign_brief.json"


def write_campaign_brief(base: Path, *, node: str, utc: str,
                         repo_root: str | Path | None = None) -> Path:
    path = brief_path(node, repo_root)
    _write_json(path, campaign_brief(base, node=node, utc=utc))
    return path


# ─────────────────────────────────────────────────────────────────────────────
# 캠페인 수명 상태 — 보류·재개·포기·선점 (2026-10-17 · plan_26101719 · interview_20261017_093905)
#
# 왜: 수명이 ACTIVE 포인터·디렉터리 존재·`campaign_closed` 줄 셋으로만 표현돼 있었다. 그래서 진행 중인
#   A 를 두고 성격이 다른 B 로 넘어갈 정식 경로가 없었고(close/purge 는 "다 끝났다" 를 요구한다),
#   `--purge-previous` 없이 `--init B` 를 치면 ACTIVE 만 덮이고 A 가 이름 없는 잔재가 되는 허점이 열려 있었다.
#
# 단일 권위 = journey 이벤트(D1). 상태는 여기서 **fold** 로 파생하고 별도 상태 필드를 손으로 적지 않는다 —
#   두 자리에 적으면 갈라지고, 갈라진 쪽이 조용히 늦는다. 상태와 종결 사유는 분리한다(GitHub state/state_reason ·
#   Jira status/resolution 선례). 쓰는 문은 이 파일 하나다.
#
# 재질의 계기는 둘뿐이다(D4): (a) 파생 자식의 종결 → 부모에 `campaign_resume_pending` · (d) 무관한 캠페인 개설
#   → 답 없는 보류분의 처분. "이미 답함" 의 단위는 (캠페인, 마지막 상태 전이)다(D5). 리볼빙·새 세션·시간 경과는
#   계기가 아니다 — 같은 질문을 세션마다 다시 묻는 것이 이 절이 막는 결함이다.

LIFECYCLE_V = 1
LIFECYCLE_KINDS = ("campaign_opened", "campaign_suspended", "campaign_resume_pending",
                   "campaign_query_answered", "campaign_resumed", "campaign_closed",
                   "cell_interrupted", "sub_control_echo", "sub_control_accepted",
                   "campaign_assignments_revised")
TRANSITION_KINDS = ("campaign_opened", "campaign_suspended", "campaign_resume_pending",
                    "campaign_resumed", "campaign_closed")
CLOSE_REASONS = ("completed", "abandoned")
# 서브에 A2A 로 전하는 수명 제어(2026-10-17 · plan_26101722 K1). 종결 둘은 사유로 갈린다 — 메인이 닫았는데 서브 ACTIVE 가
# 남으면 다음 서브 개설이 옛 인스턴스에 막히고 orphan 이 쌓인다(E2E 착수 시 서브 orphan 10 실측).
SUB_CONTROLS = ("suspend", "resume", "close", "abandon", "reslice")
CLOSE_CONTROL = {"completed": "close", "abandoned": "abandon"}
LIFECYCLE_STATES = ("active", "suspended", "resume_pending", "closed", "orphan", "unknown")
# 진행성 쓰기(D13) — 보류·미결·잔재 캠페인에서는 거부한다. 증거 정리와 읽기는 허용한다(abandoned 축소 게이트가
# 증거 정리를 요구하므로 그것까지 막으면 교착이다).
PROGRESS_OPS = ("phase_set", "cell_set", "cell_materialize", "reentry_decide", "assign_set")
# 정식 teardown 진입점(D8 · compose down 직접 ✗). 토폴로지가 진입점을 가른다 — 한 스크립트로 두 존재를 덮지 않는다.
SERVE_DOWN = {"single": (".claude/skills/upstream-version-watch/scripts/single_serve_down.sh", ()),
              "multi": (".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh", ("--down",))}
RELEASE_NONE = "none"


class TransitionRefusal(WriterRefusal):
    """수명 전이를 거부했다. 안내문이 막힘 분류(정상 차단 · 침묵 누락 · 오배달)를 말한다."""


def _is_lifecycle(entry) -> bool:
    if not isinstance(entry, dict):
        return False
    kind = str(entry.get("kind") or "")
    return kind in LIFECYCLE_KINDS or kind.startswith("campaign_") or "lifecycle_v" in entry


def _declared_topology(base: Path) -> "str | None":
    doc = read_declaration(base)
    revs = doc.get("revisions") if isinstance(doc.get("revisions"), list) else []
    cur = (revs[-1].get("values") if revs and isinstance(revs[-1], dict) else None) \
        or doc.get("control_variables") or {}
    topo = cur.get("topology") if isinstance(cur, dict) else None
    return topo if topo in ("single", "multi") else None


def _sub_nodes(base: Path) -> list:
    """A2A 로 보류·재개를 전해야 하는 서브(D9). multi 의 ray-worker 는 독립 캠페인을 갖지 않으므로 대상이 아니다."""
    if _declared_topology(base) != "single":
        return []
    # 서브 자기 인스턴스(메인 파생 선언 · self_role=sub)에는 자기 자신만 있다 — 자기에게 echo 를 기다리면 영원히
    # '서브 미수렴' 이다(2026-10-17 라이브 E2 · 서브 첫 줄이 자기 자신을 미수렴으로 보였다).
    if read_declaration(base).get("self_role") == "sub" or checkout_is_sub():
        return []
    return [n.get("node_id") for n in (read_declaration(base).get("nodes") or [])
            if isinstance(n, dict) and n.get("role") == "sub" and isinstance(n.get("node_id"), str)
            and FILL not in n.get("node_id")]


def fold_lifecycle(base: Path, *, active: "str | None" = None) -> dict:
    """journey 를 접어 수명 상태를 낸다. 부작용 없음 — 읽기만 한다.

    이벤트가 하나도 없는 옛 인스턴스(D14)는 이주 없이 해석한다: `campaign_closed` 줄이 있으면 closed(completed),
    ACTIVE 가 가리키면 active, 둘 다 아니면 orphan(재개 ✗ · abandoned 로만 정리). 모르는 이벤트(D15 — 브랜치 간
    이 파일의 버전 차)를 만나면 unknown 이며, 그 캠페인에 대한 쓰기는 전부 거부된다.
    """
    camp = base.name
    active = active_campaign_id() if active is None else active
    out = {"campaign_id": camp, "state": None, "close_reason": None, "parent": None,
           "topology": _declared_topology(base), "suspended_pins": None, "released_resources": [],
           "interrupted_cells": [], "last_transition_event": None, "answered": False,
           "pending_query": None, "sub_convergence": None, "sub_nodes": [], "sub_echoed": [],
           "handoff_pointers": [], "parked_devlog": None, "open_pending": None,
           "sub_accepted": [], "reslice_pending": {}, "unknown_events": [], "source": f"{_rel(base / JOURNEY_NAME)} fold"}
    control = None
    echoed: set = set()
    for i, e in enumerate(read_journey(base)):
        if not _is_lifecycle(e):
            continue
        kind = e.get("kind")
        try:
            ver = int(e.get("lifecycle_v") or 1)
        except (TypeError, ValueError):
            ver = LIFECYCLE_V + 1
        if kind not in LIFECYCLE_KINDS or ver > LIFECYCLE_V:
            out["unknown_events"].append({"index": i, "kind": kind, "lifecycle_v": e.get("lifecycle_v")})
            continue
        if kind == "cell_interrupted":
            if e.get("cell_id") and e["cell_id"] not in out["interrupted_cells"]:
                out["interrupted_cells"].append(e["cell_id"])
            continue
        if kind == "campaign_assignments_revised":
            # 재배정은 상태 전이가 아니다 — 서브 몫이면 그 rev 의 echo 가 올 때까지 미수렴으로 둔다(K8).
            if e.get("sub_target") and e.get("node"):
                out["reslice_pending"][e["node"]] = e.get("rev")
            continue
        if kind == "sub_control_echo" and e.get("control") == "reslice":
            if e.get("node") in out["reslice_pending"] and (e.get("rev") or -1) >= (out["reslice_pending"][e["node"]] or 0):
                out["reslice_pending"].pop(e["node"])
            continue
        if kind in ("sub_control_echo", "sub_control_accepted"):
            # 수용(accepted)은 서브 미도달을 사람이 받아들인 기록이다 — echo 와 같은 자리를 채우되 따로 남긴다(K3).
            if e.get("control") == control and e.get("node"):
                echoed.add(e["node"])
                if kind == "sub_control_accepted":
                    out["sub_accepted"].append(e["node"])
            continue
        if kind == "campaign_query_answered":
            last = out["last_transition_event"] or {}
            if e.get("answers") == last.get("ref"):
                out["answered"] = True
                if out["state"] == "resume_pending" and e.get("decision") == "keep":
                    out["state"] = "suspended"
                    out["pending_query"] = None
            continue
        if kind == "campaign_opened":
            out["state"] = "active"
            out["parent"] = e.get("parent")
        elif kind == "campaign_suspended":
            out["state"] = "suspended"
            out["suspended_pins"] = e.get("pins")
            out["released_resources"] = list(e.get("released") or [])
            out["parked_devlog"] = e.get("parked_devlog")
            out["open_pending"] = e.get("next")
            out["sub_nodes"] = list(e.get("sub_nodes") or [])
            control, echoed = "suspend", set()
        elif kind == "campaign_resume_pending":
            out["state"] = "resume_pending"
            out["pending_query"] = {"trigger": e.get("trigger"), "child": e.get("child"),
                                    "child_reason": e.get("child_reason"),
                                    "pointers": list(e.get("pointers") or []), "utc": e.get("utc")}
        elif kind == "campaign_resumed":
            out["state"] = "active"
            out["interrupted_cells"] = []
            out["handoff_pointers"] = list(e.get("handoff_pointers") or [])
            out["open_pending"] = None
            out["pending_query"] = None
            out["sub_nodes"] = list(e.get("sub_nodes") or [])
            control, echoed = "resume", set()
        elif kind == "campaign_closed":
            out["state"] = "closed"
            out["close_reason"] = e.get("reason") or "completed"
            out["open_pending"] = None
            out["pending_query"] = None
            # 종결 줄이 sub_nodes 를 싣지 않으면(lifecycle 이전 종결 · plan_26101719 시절) 전파 대상이 없다 — 소급 차단 ✗.
            out["sub_nodes"] = list(e.get("sub_nodes") or [])
            out["sub_accepted"] = []
            control, echoed = CLOSE_CONTROL.get(out["close_reason"], "close"), set()
        out["last_transition_event"] = {"ref": f"{i}:{kind}", "index": i, "kind": kind,
                                        "utc": e.get("utc")}
        out["answered"] = False
    if out["state"] is None:
        out["state"] = "active" if active == camp else "orphan"
    elif out["state"] == "active" and active != camp:
        # 활성으로 접혔는데 포인터가 다른 곳을 가리킨다 = 옛 허점(처분 없는 덮어쓰기)이 남긴 잔재다.
        out["state"] = "orphan"
    if out["unknown_events"]:
        out["state"] = "unknown"
    if checkout_is_sub():
        # 이미 적힌 종결 줄이 서브를 지목하고 있어도(옛 인스턴스) 서브 체크아웃에서는 기다릴 대상이 없다 — 위 판정과 같은 이유.
        out["sub_nodes"], out["reslice_pending"] = [], {}
    if out["sub_nodes"] and out["state"] in ("active", "suspended", "resume_pending", "closed"):
        out["sub_echoed"] = sorted(echoed)
        out["sub_convergence"] = ("converged" if set(out["sub_nodes"]) <= echoed else "unconverged")
    return out


def list_instances() -> list:
    root = campaigns_dir()
    if not root.is_dir():
        return []
    return sorted(p for p in root.iterdir() if p.is_dir() and p.name not in RESERVED_IDS)


def lifecycle_table() -> dict:
    active = active_campaign_id()
    return {p.name: fold_lifecycle(p, active=active) for p in list_instances()}


def lifecycle_line(table: "dict | None" = None) -> str:
    """`--resume-brief` **첫 줄**(D5). 평시에는 묻지 않고 이 한 줄만 보인다."""
    table = lifecycle_table() if table is None else table
    active = active_campaign_id()
    susp = [f"{c}" + (f"⊂{f['parent']}" if f.get("parent") else "")
            for c, f in table.items() if f["state"] in ("suspended", "resume_pending")]
    pend = [f"{c}←{(f['pending_query'] or {}).get('child')} 종결({(f['pending_query'] or {}).get('child_reason')}): 재개?"
            for c, f in table.items() if f["state"] == "resume_pending" and not f["answered"]]
    opening = [f"{(f['open_pending'] or {}).get('id')}@{(f['open_pending'] or {}).get('topology')}"
               for f in table.values() if f["state"] == "suspended" and f.get("open_pending")
               and (f["open_pending"] or {}).get("id") not in table]
    unconv = [f"{c}@{','.join(sorted(set(f['sub_nodes']) - set(f['sub_echoed'])))}"
              for c, f in table.items() if f.get("sub_convergence") == "unconverged"]
    reslice = [f"{c}@{','.join(sorted(f['reslice_pending']))}" for c, f in table.items()
               if f.get("reslice_pending") and f["state"] not in ("closed",)]
    odd = [f"{c}={f['state']}" for c, f in table.items() if f["state"] in ("orphan", "unknown")]
    parts = [f"ACTIVE={active}", f"보류 {len(susp)}" + (f"({', '.join(susp)})" if susp else ""),
             f"미결 질의 {len(pend)}" + (f"({'; '.join(pend)})" if pend else "")]
    if opening:
        parts.append(f"개설 대기 {', '.join(opening)}")
    if unconv:
        parts.append(f"서브 미수렴 {', '.join(unconv)}")
    if reslice:
        parts.append(f"재배정 미수렴 {', '.join(reslice)}")
    if odd:
        parts.append(f"정리 대상 {', '.join(odd)}")
    return "상태: " + " · ".join(parts)


def _topology_parity_module():
    """브랜치→토폴로지 사상의 **단일 소유자**는 4자일치 술어다(`BRANCH_TO_TOPOLOGY`). 여기서 표를 다시 적지 않는다."""
    import importlib.util
    mod_path = Path(__file__).resolve().parents[3] / "policies" / "runtime" / "topology_parity.py"
    spec = importlib.util.spec_from_file_location("_topology_parity_for_campaign", mod_path)
    if spec is None or spec.loader is None or not mod_path.is_file():
        raise TransitionRefusal(f"4자일치 술어를 적재하지 못했다: {_rel(mod_path)} — 체크아웃 토폴로지를 물을 수단이 없다")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_CHECKOUT_TOPOLOGY_FN = None      # 자체검사 주입점(픽스처 저장소에는 브랜치가 없다)
_IMAGE_DIGEST_FN = None
_RELEASE_RUNNER = None


def _manifest_topology(root: Path) -> "str | None":
    """manifest 가 토폴로지 사실의 권위다(헌법 불변식). `output/<t>/manifest.yaml` 중 자기 통로와 같은 `topology:` 를
    선언한 것이 정확히 하나일 때만 그 값을 낸다 — 둘이거나 없으면 모른다(추측 ✗)."""
    found = []
    for t in ("single", "multi"):
        mpath = root / "output" / t / "manifest.yaml"
        try:
            text = mpath.read_text(encoding="utf-8")
        except OSError:
            continue
        m = re.search(r"^topology:\s*['\"]?(single|multi)['\"]?\s*(?:#.*)?$", text, re.M)
        if m and m.group(1) == t:
            found.append(t)
    return found[0] if len(found) == 1 else None


_SELF_ROLE_FN = None              # 자체검사 주입점(픽스처 저장소에는 manifest 가 없다)


def checkout_is_sub() -> bool:
    """이 체크아웃이 서브 노드인가 — manifest 의 `self_role: sub`(메인 terraforming 이 발급 · 노드 정체의 권위)로 판정한다.

    ★ 2026-10-17 라이브(plan_26101722 S9 orphan sweep): 선언의 `self_role` 만 보면 그 키가 생기기 전에 열린 서브 인스턴스
      (camp7-sub-native)가 자기 자신을 종결 echo 대상으로 세어 영구 '서브 미수렴' 이 됐다. 서브는 누구에게도 수명 제어를
      보내지 않으므로 서브 체크아웃에서는 선언과 무관하게 전파 대상이 없다."""
    if _SELF_ROLE_FN is not None:
        return _SELF_ROLE_FN() == "sub"
    for t in ("single", "multi"):
        try:
            text = (REPO_ROOT / "output" / t / "manifest.yaml").read_text(encoding="utf-8")
        except OSError:
            continue
        if re.search(r"^self_role:\s*['\"]?sub['\"]?\s*(?:#.*)?$", text, re.M):
            return True
    return False


def _self_node_id() -> str:
    """그라운딩 기록에 적을 이 체크아웃의 노드 이름. 메인 = `main` · 서브 = manifest `library.node_id`(메인이 발급한
    접근자 · 게이트웨이 키가 묶은 값과 같다). 서브인데 그 블록이 없으면 `sub` 로 적고 출처를 숨기지 않는다."""
    if not checkout_is_sub():
        return "main"
    for t in ("single", "multi"):
        try:
            text = (REPO_ROOT / "output" / t / "manifest.yaml").read_text(encoding="utf-8")
        except OSError:
            continue
        m = re.search(r"^library:\s*\n(?:[ \t]+.*\n)*?[ \t]+node_id:\s*['\"]?([a-z0-9][a-z0-9_-]*)", text, re.M)
        if m:
            return m.group(1)
    return "sub"


def checkout_topology() -> "str | None":
    if _CHECKOUT_TOPOLOGY_FN is not None:
        return _CHECKOUT_TOPOLOGY_FN()
    root = REPO_ROOT
    try:
        tp = _topology_parity_module()
    except TransitionRefusal:
        # ★ 2026-10-17 라이브(plan_26101719 S9 · 서브 첫 위임): 서브 렌더 트리에는 기초층(`.claude/policies/`)이
        #   배달되지 않는다 — 그래서 4자일치 술어가 없고, 픽스처는 이 차이를 보지 못했다. 술어가 없는 노드에서는
        #   토폴로지 사실의 권위인 manifest 를 읽는다(브랜치 이름표를 다시 적지 않는다 — 서브 브랜치는 이름도 다르다).
        found = _manifest_topology(root)
        if found:
            return found
        # manifest 가 두 통로에 다 있으면(서브는 과거 multi 워커로도 프로비저닝됐다) 서브 로컬 git 의 브랜치를 본다.
        # 그 브랜치 이름은 sync_to_sub 가 만든 `single`/`multi` — **토폴로지 토큰 자체**라 이름표 사상이 아니다.
        import subprocess
        try:
            br = subprocess.run(["git", "-C", str(root), "symbolic-ref", "--quiet", "--short", "HEAD"],
                                capture_output=True, text=True, timeout=10).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            br = ""
        return br if br in ("single", "multi") else None
    branch = tp._git(root, "symbolic-ref", "--quiet", "--short", "HEAD") \
        or tp._git(root, "rev-parse", "--abbrev-ref", "HEAD")
    return tp.BRANCH_TO_TOPOLOGY.get(branch or "")


def _image_digest(tag: str) -> "str | None":
    if _IMAGE_DIGEST_FN is not None:
        return _IMAGE_DIGEST_FN(tag)
    import subprocess
    try:
        out = subprocess.run(["docker", "image", "inspect", "--format", "{{.Id}}", tag],
                             capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return (out.stdout.strip() or None) if out.returncode == 0 else None


def capture_pins(base: Path) -> dict:
    """보류 당시의 핀(D11). 이미지 digest 는 git 이 바이트를 들지 않는 입력이라 여기 적는 것이 유일한 증거다
    (policy:GIT_SINGLE_AUTHORITY Q1·Q2 — 맹점층)."""
    decl = read_declaration(base)
    revs = decl.get("revisions") if isinstance(decl.get("revisions"), list) else []
    cur = (revs[-1].get("values") if revs and isinstance(revs[-1], dict) else None) \
        or decl.get("control_variables") or {}
    images = []
    cells = base / "cells"
    for cdir in sorted(cells.iterdir()) if cells.is_dir() else []:
        st = _read_json(cdir / "cell.status.json")
        img = (st or {}).get("image") if isinstance(st, dict) else None
        if isinstance(img, dict) and img.get("tag") and img.get("digest"):
            entry = {"tag": img["tag"], "digest": img["digest"]}
            if entry not in images:
                images.append(entry)
    return {"topology": _declared_topology(base), "vllm_version": cur.get("vllm_version"),
            "images": images, "source": "campaign.yaml(현행 개정) · cells/*/cell.status.json#image"}


def pin_drift(pins: "dict | None") -> list:
    """재개 시 핀 대조. 토폴로지는 여기서 보지 않는다 — 그것은 drift 가 아니라 불변식 위반이다(D11·Q13-b)."""
    drift = []
    for img in (pins or {}).get("images") or []:
        now = _image_digest(img["tag"])
        if now is None:
            drift.append(f"이미지 {img['tag']}: 현재 digest 를 확인할 수 없다(로컬 부재 또는 docker 불가) — 보류 당시 {img['digest']}")
        elif now != img["digest"]:
            drift.append(f"이미지 {img['tag']}: 보류 당시 {img['digest']} → 현재 {now}")
    return drift


def running_cells(base: Path) -> list:
    """phase 가 running 인 셀 = 진행 중 셀(D7). 협조 전환은 이것이 비어 있을 때만 열린다."""
    out = []
    phases = base / "phases"
    for ndir in sorted(phases.iterdir()) if phases.is_dir() else []:
        if not ndir.is_dir() or ndir.name == "_node":
            continue
        for ph in PHASE_NAMES:
            st = _read_json(ndir / f"{ph}.status.json")
            if isinstance(st, dict) and st.get("state") == "running":
                out.append({"node": ndir.name, "phase": ph, "cell_id": st.get("cell_id")})
    return out


def release_resources(topology: "str | None", releases: list, *, apply: bool) -> list:
    """보류 시 자원 반납(D8). 정식 teardown 스크립트만 부른다. 하나라도 실패하면 거부 — 호출부는 아무것도 쓰지 않는다."""
    if not releases:
        raise TransitionRefusal(
            "보류는 자원을 반납한다 — `--release <config>`(반복) 로 이 캠페인이 띄운 서빙을 지목하거나, "
            f"띄운 것이 없으면 `--release {RELEASE_NONE}` 으로 그 사실을 선언하라(정상 차단: 침묵 skip 은 "
            "고아 서빙을 남긴다)")
    if releases == [RELEASE_NONE]:
        return [{"config": None, "status": "none-declared"}]
    if RELEASE_NONE in releases:
        raise TransitionRefusal(f"`--release {RELEASE_NONE}` 은 다른 --release 와 함께 올 수 없다(의도 모순)")
    if topology not in SERVE_DOWN:
        raise TransitionRefusal(f"캠페인 토폴로지가 선언되지 않았다({topology!r}) — 어느 teardown 진입점을 쓸지 고를 수 없다")
    script, extra = SERVE_DOWN[topology]
    out = []
    for cfg in releases:
        cmd = ["bash", str(Path(__file__).resolve().parents[4] / script), cfg, *extra]
        if not apply:
            out.append({"config": cfg, "status": "dry-run", "command": " ".join(cmd[1:])})
            continue
        if _RELEASE_RUNNER is not None:
            rc, tail = _RELEASE_RUNNER(cmd)
        else:
            import subprocess
            proc = subprocess.run(cmd, capture_output=True, text=True)
            rc, tail = proc.returncode, (proc.stdout + proc.stderr)[-600:]
        if rc != 0:
            raise TransitionRefusal(
                f"teardown 실패(rc={rc}) — {cfg}. 전환은 fail-closed 다: ACTIVE 와 journey 는 바뀌지 않았다. "
                f"자원을 정리한 뒤 다시 전환하라(정상 차단 · 우회 ✗).\n  {tail.strip()[-400:]}")
        out.append({"config": cfg, "status": "released", "script": script, "rc": rc})
    return out


def _lifecycle_event(kind: str, utc: str, **kv) -> dict:
    return {"utc": utc, "kind": kind, "lifecycle_v": LIFECYCLE_V, "source": "campaign_init", **kv}


def _devlog_name(utc: str, topic: str) -> "tuple[Path, str]":
    """명명 SSOT(wiki-desk `src/doc_naming.ts`)를 부른다 — 사서는 두 토폴로지 모두에 가므로 서브에서도 같은 정본이다
    (2026-10-19 · plan_26101912 D6). 실패는 사유째 거부한다(이름을 손으로 조립하지 않는다)."""
    import subprocess
    ts = Path(__file__).resolve().parents[2] / "wiki-desk" / "src" / "doc_naming.ts"
    ddir = REPO_ROOT / "docs" / "devlog"
    existing = sorted(p.name for p in ddir.iterdir()) if ddir.is_dir() else []
    if not ts.is_file():
        raise TransitionRefusal(f"문서 명명 SSOT 를 적재하지 못했다: {_rel(ts)}")
    cp = subprocess.run(["node", str(ts), "dated_doc_basename",
                         json.dumps(["devlog", utc, topic, existing], ensure_ascii=False)],
                        capture_output=True, text=True, check=False)
    try:
        out = json.loads(cp.stdout)
    except ValueError:
        out = {}
    if not out.get("ok"):
        raise TransitionRefusal(f"문서 명명 SSOT 실패(rc={cp.returncode}): {out.get('message') or cp.stderr.strip()[-200:]}")
    name = out["value"]
    return ddir / name, name


def write_parked_devlog(camp: str, event: dict, *, apply: bool) -> "str | None":
    """보류 시 docs 평면에 PARKED devlog 1건(D18). 인스턴스는 휘발이라 소실되면 재개 약속도 함께 사라진다 —
    사람이 다시 세울 수 있게 서사의 최소분을 문서 평면에 남긴다."""
    slug = re.sub(r"[^A-Za-z0-9_.]", "_", camp)
    path, name = _devlog_name(event["utc"], f"PARKED_캠페인보류_{slug}")
    if not apply:
        return _rel(path)
    released = ", ".join(str(r.get("config")) for r in event.get("released") or []) or "없음"
    nxt = event.get("next") or {}
    body = [
        f"# {name[:-3]}",
        "",
        f"PARKED: 캠페인 `{camp}` 보류 — {event.get('reason') or '(사유 미기재)'}",
        "",
        f"- 시각(UTC): {event['utc']} · 승인: {event.get('approved_by')}",
        f"- 선점한 캠페인: {event.get('preempted_by') or '(없음 — 단독 보류)'}"
        + (f" · 개설 대기 {nxt.get('id')}@{nxt.get('topology')}" if nxt else ""),
        f"- 내린 자원: {released}",
        f"- 중단된 셀(재개 시 pending 으로 복귀): {', '.join(event.get('interrupted_cells') or []) or '없음'}",
        f"- 보류 당시 핀: `{json.dumps(event.get('pins'), ensure_ascii=False)}`",
        f"- 서브(A2A 보류 대상): {', '.join(event.get('sub_nodes') or []) or '없음'}",
        "",
        "## 재개",
        "",
        f"- 재개는 `campaign_init.py --resume-campaign {camp} --approved-by <발화 전사> --utc <UTC> --apply`.",
        "- 재개 게이트: 보류 당시 핀·이미지 digest 대조(어긋나면 `--accept-drift` 또는 abandoned) · 토폴로지는 체크아웃과 같아야 한다(drift ✗).",
        f"- 권위는 `campaigns/{camp}/journey.jsonl` 이다 — 이 문서는 인스턴스가 사라졌을 때 사람이 재구성하기 위한 서사다.",
        "",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(body), encoding="utf-8")
    return _rel(path)


def _require(utc, approved_by, what: str) -> None:
    if not utc:
        raise TransitionRefusal(f"{what} 는 --utc 가 필요하다(시각은 주입만 받는다 — 벽시계 금지)")
    if not approved_by:
        raise TransitionRefusal(f"{what} 는 --approved-by 가 필요하다(선언 확인 팝업의 사람 발화 전사 — 승인 없는 전이 ✗)")


def _pending_dispositions(table: dict, *, exclude: tuple = ()) -> list:
    """계기 (d) — 무관한 캠페인 개설 때 처분을 물어야 하는 보류분: 마지막 전이 뒤에 답이 없는 것만(D5)."""
    return [c for c, f in table.items()
            if c not in exclude and f["state"] in ("suspended", "resume_pending") and not f["answered"]]


def _apply_dispositions(dispositions: dict, need: list, *, utc: str, approved_by: str,
                        table: dict, apply: bool) -> list:
    missing = [c for c in need if c not in dispositions]
    if missing:
        raise TransitionRefusal(
            "무관한 캠페인 개설(계기 d) — 답이 없는 보류 캠페인의 처분이 필요하다: "
            + ", ".join(missing)
            + ". 선언 확인 팝업에서 묻고 `--dispose <id>=keep` 로 전사하라. 포기는 먼저 `--abandon <id>` 로 "
              "종결 사유를 받는다(정상 차단 — 묻지 않고 넘어가면 보류가 잊힌다)")
    wrote = []
    for camp, decision in dispositions.items():
        if decision != "keep":
            raise TransitionRefusal(f"--dispose 는 keep 만 받는다({camp}={decision!r}) — 포기는 `--abandon {camp}` 로 종결한다")
        f = table.get(camp)
        if f is None or f["state"] not in ("suspended", "resume_pending"):
            raise TransitionRefusal(f"--dispose 대상 {camp} 은 보류 상태가 아니다({(f or {}).get('state')})")
        if apply:
            wrote.append(_rel(writer_append_journey(campaigns_dir() / camp, _lifecycle_event(
                "campaign_query_answered", utc, decision="keep", trigger="unrelated_open",
                answers=(f["last_transition_event"] or {}).get("ref"), approved_by=approved_by))))
    return wrote


def suspend_campaign(camp: str, *, utc, approved_by, reason, releases: list, preempt: bool,
                     preempted_by: "str | None" = None, nxt: "dict | None" = None,
                     apply: bool) -> "tuple[dict, list]":
    """보류 한 건. 검사를 전부 끝낸 뒤 teardown → 쓰기 순서다(teardown 실패 = 무쓰기)."""
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise TransitionRefusal(f"인스턴스가 없다: campaigns/{camp}")
    f = fold_lifecycle(base)
    if f["state"] != "active":
        raise TransitionRefusal(f"{camp} 은 {f['state']} 다 — 보류는 active 캠페인에만 한다")
    run = running_cells(base)
    if run and not preempt:
        raise TransitionRefusal(
            "진행 중인 셀이 있다 — " + ", ".join(f"{r['cell_id']}({r['node']}:{r['phase']})" for r in run)
            + ". 협조 전환이 기본이다: 셀을 끝낸 뒤 전환하거나, 사람이 강제를 고르면 `--preempt` 로 다시 부른다"
              "(측정 중단 · 서빙 내림 · 그 셀은 interrupted — 부분 측정값은 증거로 쓰지 않는다). 정상 차단이다")
    released = release_resources(f["topology"], releases, apply=apply)
    event = _lifecycle_event("campaign_suspended", utc, approved_by=approved_by, reason=reason,
                             preempted_by=preempted_by, preempt=bool(run and preempt),
                             released=released, pins=capture_pins(base),
                             interrupted_cells=sorted({r["cell_id"] for r in run if r.get("cell_id")}),
                             sub_nodes=_sub_nodes(base), next=nxt)
    event["parked_devlog"] = write_parked_devlog(camp, event, apply=apply)
    wrote = []
    if apply:
        for r in run:
            writer_set_phase(base, node=r["node"], phase=r["phase"], state="failed",
                             predicate=f"preempted — {preempted_by or '단독 보류'} 로 강제 전환",
                             ok=False, source="campaign_init --preempt", cell_id=r.get("cell_id"),
                             started_utc=None, ended_utc=utc)
            if r.get("cell_id"):
                writer_append_journey(base, _lifecycle_event("cell_interrupted", utc, cell_id=r["cell_id"],
                                                             node=r["node"], phase=r["phase"],
                                                             preempted_by=preempted_by))
        wrote.append(_rel(writer_append_journey(base, event)))
        if active_campaign_id() == camp:
            ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
        if event["parked_devlog"]:
            wrote.append(event["parked_devlog"])
    return event, wrote


def _parent_on_close(base: Path, *, reason: str, utc: str) -> "str | None":
    """계기 (a) — 파생 자식이 닫히면 보류 중인 부모에 재개 미결을 단다(completed·abandoned 모두 · Q9-b)."""
    f = fold_lifecycle(base, active=BOOTSTRAP)
    parent = f.get("parent")
    if not parent:
        return None
    pbase = campaigns_dir() / parent
    if not pbase.is_dir():
        return None
    pf = fold_lifecycle(pbase, active=BOOTSTRAP)
    if pf["state"] != "suspended":
        return None
    ep = _read_json(base / "evidence_pointers.json")
    ptrs = [p.get("path") for p in ((ep or {}).get("pointers") or []) if isinstance(p, dict)
            and p.get("kind") in ("testlog", "bench_report", "sweep_map", "relay_summary") and p.get("path")]
    writer_append_journey(pbase, _lifecycle_event(
        "campaign_resume_pending", utc, trigger="child_closed", child=base.name,
        child_reason=reason, pointers=ptrs))
    return parent


def abandoned_gate_reasons(camp: str, summary_testlog: "str | None") -> list:
    """abandoned 축소 게이트(D10): 측정된 셀의 증거가 docs 평면에 도착했는가 + 포기 요약 testlog 1건. P1~P3 미완분은 면제."""
    base = campaigns_dir() / camp
    reasons = []
    if not summary_testlog:
        reasons.append("포기 요약 testlog 가 없다 — `--summary-testlog <docs/testlog/...md>`(포기 사유·미완 셀 목록)")
    elif not (REPO_ROOT / summary_testlog).is_file():
        reasons.append(f"포기 요약 testlog 가 실재하지 않는다: {summary_testlog}")
    ep = _read_json(base / "evidence_pointers.json")
    ptrs = [p for p in ((ep or {}).get("pointers") or []) if isinstance(p, dict)]
    cells = base / "cells"
    for cdir in sorted(cells.iterdir()) if cells.is_dir() else []:
        st = _read_json(cdir / "cell.status.json")
        if not (isinstance(st, dict) and st.get("cell_outcome") == "measured"):
            continue
        mine = [p for p in ptrs if p.get("cell_id") == cdir.name and p.get("path")
                and FILL not in str(p.get("path"))]
        if not mine:
            reasons.append(f"측정된 셀 {cdir.name} 의 증거 포인터가 없다 — 측정은 docs 평면에 도착해야 지울 수 있다")
        for p in mine:
            if not (REPO_ROOT / p["path"]).exists():
                reasons.append(f"측정된 셀 {cdir.name} 의 증거가 실재하지 않는다: {p['path']}")
    return reasons


def abandon_campaign(camp: str, *, utc, approved_by, reason, summary_testlog,
                     releases: list, apply: bool) -> list:
    _require(utc, approved_by, "--abandon")
    if not reason:
        raise TransitionRefusal("--abandon 은 --reason 이 필요하다(사유 없는 포기는 종결이 아니라 증발이다)")
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise TransitionRefusal(f"인스턴스가 없다: campaigns/{camp}")
    f = fold_lifecycle(base)
    if f["state"] in ("closed", "unknown"):
        raise TransitionRefusal(f"{camp} 은 {f['state']} 다 — 포기할 수 없다"
                                + (" (모르는 이벤트: 브랜치 공통층 동기화 필요)" if f["state"] == "unknown" else ""))
    released = []
    if f["state"] == "active":
        if running_cells(base):
            raise TransitionRefusal("진행 중인 셀이 있다 — 셀을 끝내거나 먼저 `--suspend --preempt` 로 내린 뒤 포기하라")
        released = release_resources(f["topology"], releases, apply=apply)
    reasons = abandoned_gate_reasons(camp, summary_testlog)
    if reasons:
        raise TransitionRefusal("abandoned 축소 게이트가 닫혀 있다:\n  - " + "\n  - ".join(reasons))
    if not apply:
        return [f"dry-run abandon {camp}"]
    # 포기 요약은 메인이 쓰는 서사다 — 노드가 둘 이상이면 증거는 귀속 노드를 요구하므로(P2 공허 통과 방지) main 노드로
    # 귀속한다(2026-10-17 라이브 E7: 2노드 A 의 포기가 --node 부재로 거부됐다).
    _main = next((n.get("node_id") for n in (read_declaration(base).get("nodes") or [])
                  if isinstance(n, dict) and n.get("role") == "main" and FILL not in str(n.get("node_id"))), None)
    writer_add_evidence(base, kind="testlog", path_rel=summary_testlog, cell_id=None, node=_main, unfreeze=True)
    subs = _sub_nodes(base)
    writer_append_journey(base, _lifecycle_event("campaign_closed", utc, reason="abandoned",
                                                 note=reason, approved_by=approved_by,
                                                 summary_testlog=summary_testlog, released=released,
                                                 sub_nodes=subs,
                                                 answers=(f["last_transition_event"] or {}).get("ref")))
    if active_campaign_id() == camp:
        ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
    out = [f"closed(abandoned) {camp}" + _sub_close_hint(camp, subs, "abandon")]
    parent = _parent_on_close(base, reason="abandoned", utc=utc)
    if parent:
        out.append(f"부모 {parent} 에 재개 미결(계기 a)")
    return out


def answer_query(camp: str, *, decision: str, utc, approved_by, apply: bool) -> list:
    """계기 질의에 대한 '보류 유지' 답(keep). 재개·포기는 각자의 전이 명령이 곧 답이다."""
    _require(utc, approved_by, "--answer-query")
    if decision != "keep":
        raise TransitionRefusal("--answer-query 는 keep 만 받는다 — 재개는 --resume-campaign, 포기는 --abandon")
    base = campaigns_dir() / camp
    f = fold_lifecycle(base)
    if f["state"] not in ("suspended", "resume_pending"):
        raise TransitionRefusal(f"{camp} 은 {f['state']} 다 — 답할 보류 질의가 없다")
    if f["answered"]:
        return [f"no-op — {camp} 의 마지막 전이({(f['last_transition_event'] or {}).get('ref')})는 이미 답했다(멱등)"]
    if apply:
        writer_append_journey(base, _lifecycle_event(
            "campaign_query_answered", utc, decision="keep", approved_by=approved_by,
            trigger=(f["pending_query"] or {}).get("trigger") or "explicit",
            answers=(f["last_transition_event"] or {}).get("ref")))
    return [f"answered keep {camp}"]


def resume_campaign(camp: str, *, utc, approved_by, accept_drift: bool, apply: bool) -> list:
    _require(utc, approved_by, "--resume-campaign")
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise TransitionRefusal(f"인스턴스가 없다: campaigns/{camp}")
    f = fold_lifecycle(base)
    if f["state"] not in ("suspended", "resume_pending"):
        raise TransitionRefusal(f"{camp} 은 {f['state']} 다 — 재개는 보류 캠페인에만 한다"
                                + (" (orphan 은 재개할 수 없다 — `--abandon` 으로만 정리)" if f["state"] == "orphan" else ""))
    # 토폴로지를 슬롯보다 먼저 묻는다 — 반대 브랜치에서는 슬롯을 비워도 재개할 수 없으므로, 슬롯 안내를 먼저 내면
    # 사람이 쓸모없는 보류부터 하게 된다(안내문이 분류를 잘못 말하는 형태).
    here = checkout_topology()
    want = (f["suspended_pins"] or {}).get("topology") or f["topology"]
    if want != here:
        raise TransitionRefusal(
            f"토폴로지 불일치 — {camp} 은 topology={want!r} 인데 체크아웃은 {here!r} 다. 이것은 drift 가 아니라 "
            f"4자일치 불변식 위반이다(--accept-drift 로 넘길 수 없다). 미커밋 0 을 확인하고 해당 브랜치로 전환한 뒤 "
            f"재개하라(오배달 방지)")
    cur = active_campaign_id()
    if cur != BOOTSTRAP:
        raise TransitionRefusal(f"활성 캠페인 {cur} 이 있다 — 먼저 그것을 보류하거나 종결하라(ACTIVE 슬롯은 하나다)")
    drift = pin_drift(f["suspended_pins"])
    if drift and not accept_drift:
        raise TransitionRefusal("보류 동안 환경이 바뀌었다:\n  - " + "\n  - ".join(drift)
                                + "\n  → 사람이 수용하면 `--accept-drift`, 아니면 `--abandon`(정상 차단)")
    if not apply:
        return [f"dry-run resume {camp}" + (f" (drift 수용 {len(drift)}건)" if drift else "")]
    pq = f["pending_query"] or {}
    for cell in f["interrupted_cells"]:
        writer_set_cell(base, cell=cell, outcome="pending", node=None, version=None, model=None,
                        decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None,
                        next_intent=f"preempt 로 중단된 셀 — 재개({utc})에 처음부터 재실행", utc=utc)
    writer_append_journey(base, _lifecycle_event(
        "campaign_resumed", utc, approved_by=approved_by, accepted_drift=drift if drift else [],
        handoff_pointers=([{"child": pq.get("child"), "child_reason": pq.get("child_reason"),
                            "pointers": pq.get("pointers")}] if pq.get("child") else []),
        requeued_cells=list(f["interrupted_cells"]), sub_nodes=_sub_nodes(base),
        answers=(f["last_transition_event"] or {}).get("ref")))
    ACTIVE_POINTER.write_text(camp + "\n", encoding="utf-8")
    out = [f"resumed {camp} → ACTIVE={camp}"]
    if _sub_nodes(base):
        out.append("서브 재개는 A2A 제어명령으로 전한다: relay.py --campaign-control resume --campaign-id "
                   f"{camp} --node <sub> … (echo 가 오기 전까지 '서브 미수렴')")
    return out


def record_sub_echo(camp: str, *, node: str, control: str, utc, apply: bool, rev: "int | None" = None) -> list:
    """서브가 자기 인스턴스에서 보류/재개/종결했다는 echo(D9 · K1). 실행자는 relay(리포트 회수 직후)다."""
    if control not in SUB_CONTROLS:
        raise TransitionRefusal(f"--sub-echo 의 control 은 {'|'.join(SUB_CONTROLS)} 다: {control!r}")
    if not utc:
        raise TransitionRefusal("--sub-echo 는 --utc 가 필요하다")
    base = campaigns_dir() / camp
    f = fold_lifecycle(base)
    if control == "reslice":
        if node not in f["reslice_pending"]:
            raise TransitionRefusal(f"{node} 에 미수렴 재배정이 없다({f['reslice_pending']})")
        if rev is None or rev < (f["reslice_pending"][node] or 0):
            raise TransitionRefusal(f"reslice echo 의 rev {rev!r} 가 최신 재배정 rev {f['reslice_pending'][node]} 보다 낡았다")
    elif node not in f["sub_nodes"]:
        raise TransitionRefusal(f"{node} 는 {camp} 의 마지막 전이가 지목한 서브가 아니다({f['sub_nodes']})")
    if apply:
        writer_append_journey(base, _lifecycle_event("sub_control_echo", utc, node=node, control=control,
                                                     **({"rev": rev} if control == "reslice" else {})))
    return [f"sub echo {camp}@{node}:{control}" + (f" rev {rev}" if control == "reslice" else "")]


def accept_sub_unconverged(camp: str, *, node: str, utc, approved_by, apply: bool) -> list:
    """서브 종결 미도달을 사람이 수용한다(K3 탈출구). 종결된 캠페인에만 — 보류·재개의 미수렴은 차단이 아니므로 수용할 것이 없다."""
    _require(utc, approved_by, "--sub-unconverged-accept")
    base = campaigns_dir() / camp
    if not base.is_dir():
        raise TransitionRefusal(f"인스턴스가 없다: campaigns/{camp}")
    f = fold_lifecycle(base)
    if f["state"] != "closed":
        raise TransitionRefusal(f"{camp} 은 {f['state']} 다 — 수용은 종결된 캠페인의 서브 미수렴에만 한다"
                                "(보류·재개 미수렴은 차단이 아니다)")
    if node not in f["sub_nodes"]:
        raise TransitionRefusal(f"{node} 는 {camp} 의 종결이 지목한 서브가 아니다({f['sub_nodes']})")
    if node in f["sub_echoed"]:
        return [f"no-op — {camp}@{node} 는 이미 수렴했다(멱등)"]
    if apply:
        writer_append_journey(base, _lifecycle_event(
            "sub_control_accepted", utc, node=node, approved_by=approved_by,
            control=CLOSE_CONTROL.get(f["close_reason"], "close")))
    return [f"accepted sub-unconverged {camp}@{node}"]


def guard_writer_ops(base: Path, ops: dict) -> None:
    """보류·미결·unknown 캠페인에 대한 쓰기 가드(D13·D15). 서브도 같은 판정을 따른다(같은 코드다).

    ★ ACTIVE 를 읽지 않는다 — 판정하는 세 상태는 journey 만으로 정해지고, `--campaign-id` 를 명시한 호출부(hint 발행기)는
      ACTIVE 를 **의도적으로** 읽지 않는 계약이다(runtime_selftest 가 ACTIVE 를 읽기 불가로 만들어 그것을 시험한다)."""
    f = fold_lifecycle(base, active=base.name)
    if f["state"] == "unknown":
        raise TransitionRefusal(
            f"{base.name} 의 journey 에 이 버전이 모르는 수명 이벤트가 있다 {f['unknown_events']} — 상태를 unknown 으로 "
            "두고 쓰기를 전부 거부한다. 브랜치 공통층을 동기화한 뒤 다시 하라(오배달 방지)")
    hit = [k for k in PROGRESS_OPS if ops.get(k)]
    # 범위는 D13 그대로다(보류·미결). closed·orphan 은 이 가드의 대상이 아니다 — 종결 뒤 hint 발행이 publish phase 를
    # 적는 기존 경로가 있고, 그것을 막으면 정당한 사후 기록이 사라진다(가드가 처방을 금지하는 이음매).
    if hit and f["state"] in ("suspended", "resume_pending"):
        raise TransitionRefusal(
            f"{base.name} 은 {f['state']} 다 — 진행성 쓰기({', '.join('--' + h.replace('_', '-') for h in hit)})를 거부한다. "
            "먼저 재개하라(`--resume-campaign`). 증거 정리(--evidence-*·--backfill-from-docs)와 읽기는 허용된다(정상 차단)")


def open_campaign(new_id: str, plan_ref: str, *, utc, approved_by, suspend_current: bool,
                  derived_from: "str | None", preempt: bool, releases: list, dispositions: dict,
                  target_topology: "str | None", reason: "str | None", purge_previous: list,
                  from_slice: "str | None", apply: bool) -> list:
    """단일 정문(D2·D16). 처분 없이 활성 캠페인을 덮는 개설은 거부하고, 보류+개설은 사람 확인 1회의 원자 전이로 묶는다.

    실행 순서 = 검사 전부 → teardown → 쓰기. 토폴로지가 체크아웃과 다르면 1단(보류 + 개설 대기)까지만 하고
    대상 브랜치로 넘어가 2단(같은 명령에서 `--suspend-current` 없이 `--init`)을 하게 한다.
    """
    cur = active_campaign_id()
    table = lifecycle_table()
    here = checkout_topology()
    msgs: list = []
    # 서브가 메인 파생 선언으로 여는 경우: 계보는 선언이 싣고 오며, 처분 질의(계기 d)는 사람이 있는 메인이 이미 했다.
    # 서브는 질의 주체가 아니므로 묻지 않는다 — 대신 활성 캠페인을 덮는 개설은 메인과 똑같이 거부된다(먼저 A2A 보류).
    sub_slice = None
    if from_slice:
        _sd = _load_slice(from_slice)
        if _sd.get("self_role") == "sub":
            sub_slice = _sd
            derived_from = derived_from or _sd.get("_lifecycle_parent")
    # ── 2단: 개설 대기를 이어받는다 ───────────────────────────────────────────────
    waiting = next((c for c, f in table.items() if f["state"] == "suspended"
                    and (f.get("open_pending") or {}).get("id") == new_id), None)
    if waiting and not suspend_current:
        want = table[waiting]["open_pending"].get("topology")
        if cur != BOOTSTRAP:
            raise TransitionRefusal(f"개설 대기 {new_id} 를 이어받으려면 ACTIVE 가 비어 있어야 한다(현재 {cur})")
        if want != here:
            raise TransitionRefusal(
                f"{new_id} 는 topology={want!r} 로 개설 대기 중인데 체크아웃은 {here!r} 다 — 대상 브랜치로 전환한 뒤 "
                "같은 명령을 다시 쳐라(4자일치 · 오배달 방지)")
        derived_from = derived_from or table[waiting]["open_pending"].get("parent")
        target_topology = want
        approved_by = approved_by or table[waiting]["open_pending"].get("approved_by")
        _require(utc, approved_by, "개설 대기 이어받기(2단)")
        need: list = []
    else:
        if cur != BOOTSTRAP and not suspend_current and cur not in purge_previous:
            raise TransitionRefusal(
                f"활성 캠페인 {cur} 이 있다 — 처분 없이 덮어쓰는 --init 은 거부한다(옛 허점: ACTIVE 만 바뀌고 {cur} 이 "
                f"이름 없는 잔재가 된다). 셋 중 하나를 고른다: ① `--suspend-current [--derived-from {cur}]` 로 보류+개설 "
                f"② `--close {cur}`(완주) ③ `--abandon {cur}`(포기) — 정상 차단")
        if suspend_current and cur == BOOTSTRAP:
            raise TransitionRefusal("--suspend-current 를 줬지만 활성 캠페인이 없다(ACTIVE=_bootstrap) — 플래그를 빼라(의도 모순)")
        if derived_from and sub_slice is None:
            pf = table.get(derived_from)
            ok_state = ("active",) if (suspend_current and derived_from == cur) else ("suspended", "resume_pending")
            if pf is None or pf["state"] not in ok_state:
                raise TransitionRefusal(f"--derived-from {derived_from} 은 {(pf or {}).get('state')} 다 — 파생 부모는 "
                                        f"지금 보류되는 활성 캠페인이거나 보류 중인 캠페인이어야 한다")
        # 계기 (d): 파생이 아니면 무관한 개설이다. 지금 보류되는 cur 는 이 전이 자체가 처분이므로 묻지 않는다.
        need = [] if (derived_from or sub_slice is not None) else _pending_dispositions(table, exclude=(cur,))
        if suspend_current or utc or approved_by or need or dispositions:
            _require(utc, approved_by, "수명 전이(--init)")
    target_topology = target_topology or here
    if target_topology not in ("single", "multi"):
        raise TransitionRefusal(f"대상 토폴로지를 정할 수 없다({target_topology!r}) — `--target-topology single|multi`")
    if new_id in table or (campaigns_dir() / new_id).exists():
        raise TransitionRefusal(f"이미 있는 인스턴스다: campaigns/{new_id}")
    if new_id in RESERVED_IDS:
        raise TransitionRefusal(f"예약 id 로는 캠페인을 열 수 없다: {new_id!r}")
    # 처분 답 누락은 teardown 전에 막는다(검사 전부 → teardown → 쓰기).
    if need:
        _apply_dispositions(dispositions, need, utc=utc, approved_by=approved_by, table=table, apply=False)
    stage1_only = target_topology != here
    if suspend_current:
        nxt = ({"id": new_id, "topology": target_topology, "plan_ref": plan_ref, "parent": derived_from,
                "approved_by": approved_by} if stage1_only else None)
        event, wrote = suspend_campaign(cur, utc=utc, approved_by=approved_by,
                                        reason=reason or f"{new_id} 개설로 전환", releases=releases,
                                        preempt=preempt, preempted_by=new_id, nxt=nxt, apply=apply)
        msgs.append(f"suspended {cur} (내린 자원 {len(event['released'])} · 중단 셀 {len(event['interrupted_cells'])})")
        msgs += wrote
    elif stage1_only:
        raise TransitionRefusal(
            f"대상 토폴로지 {target_topology!r} 가 체크아웃({here!r})과 다르다 — 반대 토폴로지 캠페인을 이 브랜치에서 "
            "열 수 없다(4자일치). 보류할 활성 캠페인이 있으면 `--suspend-current` 로 1단을 남기고, 없으면 대상 브랜치에서 연다")
    if need:
        msgs += _apply_dispositions(dispositions, need, utc=utc, approved_by=approved_by, table=table, apply=apply)
    if stage1_only:
        msgs.append(f"1단 완료 — {new_id} 개설 대기(topology={target_topology}). 미커밋 0 확인 → "
                    f"{target_topology}-node 체크아웃 → 같은 `--init {new_id} --plan-ref {plan_ref} --apply` 로 2단")
        return msgs
    removed = []
    for prev in purge_previous:
        removed += do_purge(prev, apply=apply)
    swept = sweep_bootstrap_relay(apply=apply)
    made = scaffold(new_id, plan_ref, apply=apply, from_slice=from_slice)
    if apply:
        nbase = campaigns_dir() / new_id
        doc = read_declaration(nbase)
        cv = doc.get("control_variables") if isinstance(doc.get("control_variables"), dict) else {}
        if cv.get("topology") in (None, FILL):
            cv["topology"] = target_topology
            doc["control_variables"] = cv
            (nbase / "campaign.yaml").write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n",
                                                 encoding="utf-8")
        if utc:
            writer_append_journey(nbase, _lifecycle_event("campaign_opened", utc, parent=derived_from,
                                                          topology=target_topology, approved_by=approved_by,
                                                          plan_ref=plan_ref))
    msgs.append(f"purge={removed} init={made}" + (f" parent={derived_from}" if derived_from else ""))
    if swept:
        msgs.append(f"_bootstrap/relay 정리 {len(swept)}건")
    if suspend_current and cur != BOOTSTRAP and _sub_nodes(campaigns_dir() / cur):
        msgs.append(f"서브 보류는 A2A 제어명령으로 전한다: relay.py --campaign-control suspend --campaign-id {cur} "
                    "--node <sub> … (echo 가 오기 전까지 '서브 미수렴' · 메인은 진행)")
    return msgs


def _selftest() -> int:
    import tempfile
    global CAMPAIGNS, ACTIVE_POINTER, REPO_ROOT
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

    saved = (CAMPAIGNS, ACTIVE_POINTER, REPO_ROOT)
    with tempfile.TemporaryDirectory() as tmp:
        CAMPAIGNS = Path(tmp) / "campaigns"
        ACTIVE_POINTER = CAMPAIGNS / "ACTIVE"
        prev = CAMPAIGNS / "old"
        prev.mkdir(parents=True)
        # 2026-10-17(plan_26101719 D14): 수명 이벤트 없고 ACTIVE 도 아닌 인스턴스는 orphan 이라 P-게이트 앞에서 막힌다.
        # 이 절은 P-게이트 자체를 시험하므로 픽스처를 **옛 종결(campaign_closed 줄)** 로 둔다 — orphan 판정은
        # selftest_campaign_lifecycle.py 가 따로 시험한다.
        _legacy_closed = lambda d: (d / JOURNEY_NAME).write_text(
            json.dumps({"utc": "2026-01-01T00:00:00Z", "kind": "campaign_closed"}) + "\n", encoding="utf-8")
        _legacy_closed(prev)
        ck("포인터 파일 부재 → 게이트 닫힘", bool(purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps({"pointers": []}), encoding="utf-8")
        ck("포인터 0건 → 게이트 닫힘", any("0건" in r for r in purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "does/not/exist.yaml"}]}), encoding="utf-8")
        ck("증거 부재 → 게이트 닫힘", any("실재하지 않는다" in r for r in purge_gate_reasons("old")))
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"}]}), encoding="utf-8")
        (prev / "relay").mkdir(parents=True, exist_ok=True)
        (prev / "relay" / "ctx.json").write_text("{}", encoding="utf-8")
        ck("릴레이 요약 부재 → 게이트 닫힘", any("relay_summary" in r for r in purge_gate_reasons("old")))
        (prev / "relay" / "ctx.json").unlink()
        ck("★원장이 없으면 요약을 요구하지 않는다(서브 인스턴스는 왕복 원장을 갖지 않는다)",
           not any("relay_summary" in r for r in purge_gate_reasons("old")))
        (prev / "relay" / "ctx.json").write_text("{}", encoding="utf-8")
        (prev / "evidence_pointers.json").write_text(json.dumps(
            {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"},
                          {"kind": "relay_summary", "path": _PORTABLE_EVIDENCE}]}), encoding="utf-8")
        ck("전수 실재 + 요약 → 게이트 열림", not purge_gate_reasons("old"))
        ck("dry-run 은 지우지 않는다", do_purge("old", apply=False) and prev.is_dir())
        ck("apply 는 지운다", do_purge("old", apply=True) is not None and not prev.is_dir())
        # 인스턴스가 둘 남았을 때(중단·누출 흡수) 정식 경로로 둘 다 지울 수 있는가.
        # 하나만 지워지면 남은 하나는 rm -rf 우회를 부른다.
        good = {"pointers": [{"kind": "certificate", "path": "CLAUDE.md"},
                             {"kind": "relay_summary", "path": _PORTABLE_EVIDENCE}]}
        for _n in ("relicA", "relicB"):
            _d = CAMPAIGNS / _n
            _d.mkdir(parents=True)
            _legacy_closed(_d)
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
        _legacy_closed(CAMPAIGNS / "relicC")
        _refused = False
        try:
            do_purge("relicC", apply=True)
        except PurgeGateRefusal:
            _refused = True
        ck("★음성대조: 포인터 없는 잔재는 여전히 거부된다",
           _refused and (CAMPAIGNS / "relicC").is_dir())
        # ── 종결(--close) — ACTIVE 만 내리고 인스턴스는 남긴다(2026-09-15) ──────────────
        _cl = CAMPAIGNS / "closeme"
        _cl.mkdir(parents=True)
        ACTIVE_POINTER.write_text("closeme\n", encoding="utf-8")
        _refused = False
        try:
            close_campaign("closeme", utc="2026-09-15T00:00:00Z", apply=True)
        except PurgeGateRefusal:
            _refused = True
        ck("★종결 음성대조: 증거 포인터 없는 활성 캠페인은 종결을 거부하고 ACTIVE 를 그대로 둔다",
           _refused and active_campaign_id() == "closeme")
        (_cl / "evidence_pointers.json").write_text(json.dumps(good), encoding="utf-8")
        _refused = False
        try:
            close_campaign("other", utc="2026-09-15T00:00:00Z", apply=True)
        except PurgeGateRefusal:
            _refused = True
        ck("종결 음성대조: 활성이 아닌 id 는 거부", _refused and active_campaign_id() == "closeme")
        _refused = False
        try:
            close_campaign("closeme", utc=None, apply=True)
        except PurgeGateRefusal:
            _refused = True
        ck("종결 음성대조: --utc 없으면 거부(벽시계 금지)", _refused and active_campaign_id() == "closeme")
        close_campaign("closeme", utc="2026-09-15T00:00:00Z", apply=False)
        ck("종결 dry-run 은 ACTIVE 를 바꾸지 않는다", active_campaign_id() == "closeme")
        _msg = close_campaign("closeme", utc="2026-09-15T00:00:00Z", apply=True)
        ck("종결 apply → ACTIVE=_bootstrap · 인스턴스 보존 · 여정 기재",
           active_campaign_id() == BOOTSTRAP and _cl.is_dir()
           and any(j.get("kind") == "campaign_closed" for j in read_journey(_cl)) and "closed" in _msg)
        ck("종결은 멱등이다(이미 _bootstrap 이면 no-op)",
           "no-op" in close_campaign("closeme", utc="2026-09-15T00:00:00Z", apply=True))
        ck("종결 뒤 같은 인스턴스는 purge 로 지울 수 있다",
           not purge_gate_reasons("closeme") and do_purge("closeme", apply=True) and not _cl.exists())
        # ── 단일 writer + resume-brief (2026-09-07 · plan_26090715 §4.1·§4.4) ──────────────
        camp = CAMPAIGNS / "w1"
        for sub in ("cells", "phases", "sweeps", "relay"):
            (camp / sub).mkdir(parents=True, exist_ok=True)
        (camp / "campaign.yaml").write_text(json.dumps({
            "schema_version": 1, "id": "w1", "plan_ref": "docs/plan/p.md",
            "declared_utc": "2026-01-01T00:00:00Z",
            "nodes": [{"node_id": "main", "role": "main"}, {"node_id": "sub", "role": "sub"}],
            "assignments": {"main": [{"cell": "c1"}, {"cell": "c2", "mode": "STAY"}]},
            "control_variables": {"model": "m0", "vllm_version": "0.1.0"},
        }, ensure_ascii=False), encoding="utf-8")
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")

        ck("활성 포인터가 인스턴스를 가리킨다", active_campaign_id() == "w1")
        writer_set_phase(camp, node="main", phase="build", state="done",
                         predicate="이미지 실재 + --gpus=all 프로브", ok=True,
                         source="docker inspect", cell_id="c1",
                         started_utc=None, ended_utc=None)
        _b = _read_json(camp / "phases" / "main" / "build.status.json")
        ck("phase writer 가 proof 를 쓴다",
           _b["state"] == "done" and _b["proof"]["ok"] is True and _b["proof"]["source"])

        def _boom(fn):
            try:
                fn(); return False
            except WriterRefusal:
                return True
        ck("★음성대조 proof.ok 인데 source 없으면 거부",
           _boom(lambda: writer_set_phase(camp, node="main", phase="serve", state="done",
                                          predicate="health 200", ok=True, source=None,
                                          cell_id=None, started_utc=None, ended_utc=None)))
        ck("★음성대조 알 수 없는 phase 거부",
           _boom(lambda: writer_set_phase(camp, node="main", phase="nope", state="done",
                                          predicate="x", ok=False, source=None, cell_id=None,
                                          started_utc=None, ended_utc=None)))
        ck("★음성대조 알 수 없는 state 거부",
           _boom(lambda: writer_set_phase(camp, node="main", phase="serve", state="nope",
                                          predicate="x", ok=False, source=None, cell_id=None,
                                          started_utc=None, ended_utc=None)))
        ck("★음성대조 <<FILL>> 잔존 거부",
           _boom(lambda: writer_set_phase(camp, node="<<FILL>>", phase="serve", state="pending",
                                          predicate=None, ok=False, source=None, cell_id=None,
                                          started_utc=None, ended_utc=None)))

        writer_set_cell(camp, cell="c1", outcome="measured", node="main", version="0.1.0",
                        model="m0", decode_tps="12.5", measurement_source="certificate x.yaml",
                        void_reason=None, void_reason_source=None,
                        axis_citation="kv dtype 축", next_intent="c2 착수 — attn 축",
                        utc="2026-01-01T01:00:00Z")
        _c = _read_json(camp / "cells" / "c1" / "cell.status.json")
        ck("cell writer 가 outcome·측정·축을 쓴다",
           _c["cell_outcome"] == "measured" and _c["measurement"]["decode_tps_conc1"] == 12.5
           and _c["measurement"]["source"] and _c["axis_citation"] == "kv dtype 축")
        ck("여정 한 줄이 같은 트랜잭션에서 쌓인다",
           len(read_journey(camp)) == 1 and read_journey(camp)[0]["next_intent"].startswith("c2"))
        # ── lite ② → cap 차감 · 재발동 제안(2026-09-29 · plan_26092923_58_27) — 별도 인스턴스(여정 수 검사와 섞지 않는다)
        _lc = Path(tmp) / "lite_camp"; (_lc / "cells").mkdir(parents=True)

        def _lraw(name, verdict, measured):
            _p = Path(tmp) / name
            _p.write_text(json.dumps({"lite_verdict": verdict, "measured_utc": measured}), encoding="utf-8")
            return str(_p)

        def _lset(cell, raw, utc, intent=None):
            return writer_set_cell(_lc, cell=cell, outcome="measurement_void", node="main", version=None, model=None,
                                   decode_tps=None, measurement_source=None, void_reason="lite_server_failed",
                                   void_reason_source="lite_raw(x)", axis_citation=None, next_intent=intent,
                                   utc=utc, lite_raw=raw)
        _sf1 = _lraw("sf1.json", "server_failed", "2026-01-01T02:00:00Z")
        _lset("lc", _sf1, "2026-01-01T02:01:00Z", intent="재발동 판단 대기")
        _l = _read_json(_lc / "cells" / "lc" / "cell.status.json")
        ck("★LR1 lite ② → lite_gate · cap 차감 1 · 재발동 제안(proposed) · cap 값은 적지 않는다",
           _l["lite_gate"]["verdict"] == "server_failed" and _l["reconciliation"]["spent"] == 1
           and _l["reentry"]["status"] == "proposed" and "cap" not in _l["reconciliation"]
           and read_journey(_lc)[-1].get("lite_verdict") == "server_failed")
        _lset("lc", _sf1, "2026-01-01T02:05:00Z")
        ck("★LR2 같은 측정(raw measured_utc)을 다시 적어도 차감은 1(멱등)",
           _read_json(_lc / "cells" / "lc" / "cell.status.json")["reconciliation"]["spent"] == 1)
        _lset("lc", _lraw("sf2.json", "server_failed", "2026-01-01T03:00:00Z"), "2026-01-01T03:01:00Z")
        ck("LR3 새 측정의 ② 는 차감 2", _read_json(_lc / "cells" / "lc" / "cell.status.json")["reconciliation"]["spent"] == 2)
        _lset("lm", _lraw("mp.json", "measurement_path_failed", "2026-01-01T02:00:00Z"), "2026-01-01T02:01:00Z")
        _m = _read_json(_lc / "cells" / "lm" / "cell.status.json")
        ck("★LR4 ① measurement_path_failed 는 차감 0 · 제안 0(하네스 수리)",
           _m["lite_gate"]["verdict"] == "measurement_path_failed" and "reconciliation" not in _m and "reentry" not in _m)
        _lset("lv", _lraw("voc.json", "ok", "2026-01-01T02:00:00Z"), "2026-01-01T02:01:00Z")
        _v = _read_json(_lc / "cells" / "lv" / "cell.status.json")
        ck("LR5 어휘 밖 판정 → verdict None · 차감 0(소유자 규칙)", _v["lite_gate"]["verdict"] is None and "reentry" not in _v)
        ck("★LR6 음성대조 제안 없는 셀의 재발동 결정은 거부",
           _boom(lambda: writer_decide_reentry(_lc, cell="lm", decision="approve", route="explorer",
                                               approved_by="사용자", utc="2026-01-01T04:00:00Z")))
        ck("★LR7 음성대조 승인에 경로가 없으면 거부",
           _boom(lambda: writer_decide_reentry(_lc, cell="lc", decision="approve", route=None,
                                               approved_by="사용자", utc="2026-01-01T04:00:00Z")))
        ck("LR8 음성대조 발화 전사 없는 결정은 거부",
           _boom(lambda: writer_decide_reentry(_lc, cell="lc", decision="decline", route=None,
                                               approved_by=" ", utc="2026-01-01T04:00:00Z")))
        writer_decide_reentry(_lc, cell="lc", decision="approve", route="explorer",
                              approved_by="사용자(발화 전사) — 서빙전략 다시", utc="2026-01-01T04:00:00Z")
        _l = _read_json(_lc / "cells" / "lc" / "cell.status.json")
        ck("LR9 결정 기록 → approved · 경로 · 여정 한 줄",
           _l["reentry"]["status"] == "approved" and "explorer" in _l["reentry"]["decision"]["route"]
           and read_journey(_lc)[-1]["source"] == "campaign_init --reentry-decide")
        ck("LR10 결정된 제안은 다시 결정할 수 없다(대기 제안 없음)",
           _boom(lambda: writer_decide_reentry(_lc, cell="lc", decision="decline", route=None,
                                               approved_by="사용자", utc="2026-01-01T05:00:00Z")))
        _lset("lc", _lraw("sf3.json", "server_failed", "2026-01-01T06:00:00Z"), "2026-01-01T06:01:00Z")
        _l = _read_json(_lc / "cells" / "lc" / "cell.status.json")
        ck("LR11 결정 뒤 새 ② 는 새 제안을 연다(차감 3)",
           _l["reentry"]["status"] == "proposed" and _l["reconciliation"]["spent"] == 3)
        ck("★음성대조 알 수 없는 outcome 거부",
           _boom(lambda: writer_set_cell(camp, cell="c2", outcome="nope", node="main",
                                         version=None, model=None, decode_tps=None,
                                         measurement_source=None, void_reason=None,
                                         void_reason_source=None, axis_citation=None,
                                         next_intent=None, utc=None)))
        ck("★음성대조 void_reason 에 출처 없으면 거부",
           _boom(lambda: writer_set_cell(camp, cell="c2", outcome="void", node="main",
                                         version=None, model=None, decode_tps=None,
                                         measurement_source=None, void_reason="열 트립",
                                         void_reason_source=None, axis_citation=None,
                                         next_intent=None, utc=None)))
        ck("★음성대조 측정값에 출처 없으면 거부",
           _boom(lambda: writer_set_cell(camp, cell="c2", outcome="measured", node="main",
                                         version=None, model=None, decode_tps="1.0",
                                         measurement_source=None, void_reason=None,
                                         void_reason_source=None, axis_citation=None,
                                         next_intent=None, utc=None)))

        writer_add_evidence(camp, kind="testlog", path_rel="CLAUDE.md", cell_id="c1",
                            node="main", unfreeze=False)
        _e = _read_json(camp / "evidence_pointers.json")
        ck("evidence writer 가 포인터를 쓴다", len(_e["pointers"]) == 1)
        writer_add_evidence(camp, kind="testlog", path_rel="CLAUDE.md", cell_id="c1",
                            node="main", unfreeze=False)
        ck("같은 포인터는 두 번 적지 않는다(멱등)",
           len(_read_json(camp / "evidence_pointers.json")["pointers"]) == 1)
        # 비어 있는 태그는 채운다 — null node_id 를 고칠 연산이 없으면 손이 파일을 연다(F1 형태).
        _write_json(camp / "evidence_pointers.json", {"schema_version": 1, "pointers": [
            {"kind": "bench_report", "path": _PORTABLE_EVIDENCE, "cell_id": None, "node_id": None}]})
        writer_add_evidence(camp, kind="bench_report", path_rel=_PORTABLE_EVIDENCE, cell_id="c1",
                            node="main", unfreeze=False)
        ck("★비어 있는 노드 태그는 정식 경로로 채운다(손편집 대체)",
           _read_json(camp / "evidence_pointers.json")["pointers"][0]["node_id"] == "main")
        ck("★음성대조 이미 다른 노드로 귀속된 증거는 조용히 바꾸지 않는다",
           _boom(lambda: writer_add_evidence(camp, kind="bench_report", path_rel=_PORTABLE_EVIDENCE,
                                             cell_id=None, node="sub", unfreeze=False)))
        _write_json(camp / "evidence_pointers.json", {"schema_version": 1, "pointers": [
            {"kind": "testlog", "path": "CLAUDE.md", "cell_id": "c1", "node_id": "main"}]})
        ck("★음성대조 실재하지 않는 증거 거부",
           _boom(lambda: writer_add_evidence(camp, kind="testlog", path_rel="does/not/exist.md",
                                             cell_id=None, node=None, unfreeze=False)))
        ck("★음성대조 알 수 없는 kind 거부",
           _boom(lambda: writer_add_evidence(camp, kind="nope", path_rel="CLAUDE.md",
                                             cell_id=None, node=None, unfreeze=False)))
        writer_add_evidence(camp, kind="simlog", path_rel="CLAUDE.md", cell_id=None,
                            node="main", unfreeze=False)
        _stub = _read_json(camp / "evidence_pointers.json")
        _stub["pointers"].append({"kind": FILL, "path": FILL, "cell_id": None, "node_id": None})
        _stub["campaign_id"] = FILL
        _write_json(camp / "evidence_pointers.json", _stub)
        _n_before = len(_read_json(camp / "evidence_pointers.json")["pointers"])
        _ep, _dropped = writer_prune_stubs(camp, unfreeze=False)
        _after = _read_json(camp / "evidence_pointers.json")
        ck("★스텁 포인터를 정식 경로로 지운다(손삭제 대체 · F1 재발 방지)",
           len(_dropped) == 1 and len(_after["pointers"]) == _n_before - 1)
        ck("★값이 든 포인터는 건드리지 않는다(증거 삭제 연산이 아니다)",
           any(x.get("path") == "CLAUDE.md" for x in _after["pointers"]))
        ck("★campaign_id 의 빈칸도 함께 채운다(setdefault 는 빈칸을 못 덮는다)",
           _after["campaign_id"] == "w1")
        writer_add_evidence(camp, kind="relay_summary", path_rel="CLAUDE.md", cell_id="c1", node="main", unfreeze=False)
        ck("★음성대조 사유 없는 셀 태그 해제 거부",
           _boom(lambda: writer_untag_evidence(camp, kind="relay_summary", path_rel="CLAUDE.md", cell_id="c1",
                                               reason=" ", utc="2026-01-01T01:30:00Z", unfreeze=False)))
        ck("★음성대조 다른 셀 이름으로 해제 거부(엉뚱한 해제 ✗)",
           _boom(lambda: writer_untag_evidence(camp, kind="relay_summary", path_rel="CLAUDE.md", cell_id="c9",
                                               reason="r", utc="2026-01-01T01:30:00Z", unfreeze=False)))
        writer_untag_evidence(camp, kind="relay_summary", path_rel="CLAUDE.md", cell_id="c1", reason="셀 선행 근거",
                              utc="2026-01-01T01:30:00Z", unfreeze=False)
        _ut = [x for x in _read_json(camp / "evidence_pointers.json")["pointers"]
               if x.get("kind") == "relay_summary" and x.get("path") == "CLAUDE.md"]
        ck("★셀 태그 해제: 포인터는 남고 cell_id=None · 이력에 옛 귀속·사유·시각",
           len(_ut) == 1 and _ut[0]["cell_id"] is None
           and _ut[0]["untagged"] == [{"cell_id": "c1", "reason": "셀 선행 근거", "utc": "2026-01-01T01:30:00Z"}])
        # 경로 이전(2026-09-29 · 문서 개명) — 옛 경로 부재 ∧ 새 경로 실재일 때만
        _rl_old, _rl_new = "docs/_selftest_relocate_old.md", ".claude/rules/docs.md"
        _ptrs = _read_json(camp / "evidence_pointers.json")
        _ptrs["pointers"].append({"kind": "testlog", "path": _rl_old, "cell_id": "c1", "node_id": "main"})
        _write_json(camp / "evidence_pointers.json", _ptrs)
        ck("★음성대조 사유 없는 경로 이전 거부",
           _boom(lambda: writer_relocate_evidence(camp, kind="testlog", path_from=_rl_old, path_to=_rl_new, reason=" ",
                                                  utc="2026-01-01T01:40:00Z", unfreeze=False)))
        ck("★음성대조 새 경로 부재면 거부",
           _boom(lambda: writer_relocate_evidence(camp, kind="testlog", path_from=_rl_old, path_to="docs/_nope_.md",
                                                  reason="r", utc="2026-01-01T01:40:00Z", unfreeze=False)))
        ck("★음성대조 옛 경로가 실재하면 개명이 아니다(거부)",
           _boom(lambda: writer_relocate_evidence(camp, kind="relay_summary", path_from="CLAUDE.md", path_to="CLAUDE.md",
                                                  reason="r", utc="2026-01-01T01:40:00Z", unfreeze=False)))
        writer_relocate_evidence(camp, kind="testlog", path_from=_rl_old, path_to=_rl_new, reason="명명 SSOT 로 개명",
                                 utc="2026-01-01T01:40:00Z", unfreeze=False)
        _rl = [x for x in _read_json(camp / "evidence_pointers.json")["pointers"] if x.get("kind") == "testlog"
               and x.get("path") == _rl_new]
        ck("★경로 이전: 새 경로 · 셀 귀속 유지 · 이력(from·사유·시각)",
           len(_rl) == 1 and _rl[0]["cell_id"] == "c1"
           and _rl[0]["relocated"] == [{"from": _rl_old, "reason": "명명 SSOT 로 개명", "utc": "2026-01-01T01:40:00Z"}])
        ck("★음성대조 같은 종류 새 경로 포인터 중복이면 거부",
           _boom(lambda: writer_relocate_evidence(camp, kind="testlog", path_from=_rl_new, path_to=_rl_new, reason="r",
                                                  utc="2026-01-01T01:41:00Z", unfreeze=False)))
        writer_freeze_evidence(camp, "2026-01-01T02:00:00Z")
        ck("★음성대조 동결 뒤 추가는 거부(입력 통로가 흐르면 태그는 불변인데 근거가 움직인다)",
           _boom(lambda: writer_add_evidence(camp, kind="devlog", path_rel=_PORTABLE_EVIDENCE,
                                             cell_id=None, node=None, unfreeze=False)))
        writer_add_evidence(camp, kind="devlog", path_rel=_PORTABLE_EVIDENCE, cell_id=None,
                            node="main", unfreeze=True)
        ck("사람이 --unfreeze 를 붙이면 통과",
           any(x.get("kind") == "devlog" and x.get("path") == _PORTABLE_EVIDENCE
               for x in _read_json(camp / "evidence_pointers.json")["pointers"]))

        writer_add_revision(camp, values={"model": "m1"}, reason="타겟 모델 전환",
                            evidence="docs/testlog/t.md", approved_by="operator",
                            utc="2026-01-01T03:00:00Z")
        _y = _read_json(camp / "campaign.yaml")
        ck("revisions[0] 은 t0 원본이고 개정은 append",
           len(_y["revisions"]) == 2 and _y["revisions"][0]["values"]["model"] == "m0"
           and _y["revisions"][1]["values"]["model"] == "m1"
           and _y["control_variables"]["model"] == "m0")
        ck("★음성대조 사유·근거·승인자 없는 개정 거부",
           _boom(lambda: writer_add_revision(camp, values={"model": "m2"}, reason="",
                                             evidence="e", approved_by="o", utc="t")))

        brief = resume_brief("w1")
        ck("resume-brief 가 다음 pending 셀을 지목한다", "다음 pending 셀: c2" in brief)
        ck("resume-brief 가 최신 개정값을 보여준다", "model: m1" in brief)
        ck("resume-brief 가 여정 마지막 줄을 보여준다", "c2 착수 — attn 축" in brief)
        ck("resume-brief 가 선언 노드의 진행표 부재를 표시한다", "sub:" in brief and "phases/ 디렉터리 없음" in brief)
        ck("★resume-brief 가 셀마다 배정 노드와 모드를 보여준다(동시에 도는 리스트가 보인다)",
           "[main · AUTO]" in brief and "[main · STAY]" in brief)
        # lite ② 재발동 대기가 brief 에 보인다(2026-09-29) — 검사 뒤 셀을 지워 이후 검사와 섞지 않는다.
        writer_set_cell(camp, cell="lz", outcome="measurement_void", node="main", version=None, model=None,
                        decode_tps=None, measurement_source=None, void_reason="lite_server_failed",
                        void_reason_source="lite_raw(x)", axis_citation=None, next_intent=None,
                        utc="2026-01-01T02:01:00Z", lite_raw=_sf1)
        _bz = resume_brief("w1")
        ck("★LR12 resume-brief 가 lite ② 셀의 차감 수와 재발동 대기(사람 승인)를 보여준다",
           "lite ② · cap 차감 1 · 재발동 proposed" in _bz and "재발동 대기(사람 승인)" in _bz and "`lz`" in _bz)
        shutil.rmtree(camp / "cells" / "lz")

        # ── 측정값을 writer 가 직접 읽는다 (2026-09-08 · plan_26090813 D19) ────────────────
        (camp / "sweeps").mkdir(parents=True, exist_ok=True)
        _sw = camp / "sweeps" / "s1.json"
        _sw.write_text(json.dumps({"cells": [
            {"cell_key": "c2", "cell_outcome": "measured",
             "concurrency_vector": {"1": 19.12, "2": 19.68}}]}), encoding="utf-8")
        writer_set_cell(camp, cell="c2", outcome="measured", node=None, version=None, model=None,
                        decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None,
                        utc="t3", sweep_state=str(_sw))
        _m = _read_json(camp / "cells" / "c2" / "cell.status.json")
        ck("★writer 가 sweep 기록에서 측정값을 직접 읽는다(호출부 플래그 ✗)",
           _m["measurement"]["decode_tps_conc1"] == 19.12
           and "concurrency_vector.1" in _m["measurement"]["source"])
        ck("★셀의 노드를 배정에서 파생한다(--node 를 안 줘도 null 이 남지 않는다)",
           _m["node_id"] == "main")
        writer_set_cell(camp, cell="c3", outcome="measured", node="main", version=None, model=None,
                        decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None,
                        utc="t4", sweep_state=str(_sw))
        _g = _read_json(camp / "cells" / "c3" / "cell.status.json")
        ck("★sweep 에 그 셀이 없으면 **결손 기재**하고 통과한다(차단 ✗ · 무한 hang 경계)",
           _g["measurement"]["decode_tps_conc1"] is None and "c3" in _g["measurement"]["gap"])

        # ── 다노드에서 증거의 노드 태그는 필수다 (P2 공허 통과의 뿌리) ────────────────────
        ck("★음성대조 다노드에서 --node 없는 증거는 거부",
           _boom(lambda: writer_add_evidence(camp, kind="sweep_map", path_rel=_PORTABLE_EVIDENCE,
                                             cell_id=None, node=None, unfreeze=True)))
        writer_add_evidence(camp, kind="sweep_map", path_rel=_PORTABLE_EVIDENCE, cell_id="c1",
                            node=None, unfreeze=True)
        ck("★셀을 주면 배정에서 노드를 파생한다",
           any(x.get("kind") == "sweep_map" and x.get("node_id") == "main"
               for x in _read_json(camp / "evidence_pointers.json")["pointers"]))

        # ── 착수 시각은 처음 한 번만 굳는다(P4 가 늦은 값을 보고 통과하지 않게) ──────────
        writer_set_phase(camp, node="main", phase="serve", state="running", predicate=None,
                         ok=False, source=None, cell_id="c1",
                         started_utc="2026-01-01T10:00:00Z", ended_utc=None)
        writer_set_phase(camp, node="main", phase="serve", state="running", predicate=None,
                         ok=False, source=None, cell_id="c2",
                         started_utc="2026-01-01T20:00:00Z", ended_utc=None, authored_by="main")
        _p = _read_json(camp / "phases" / "main" / "serve.status.json")
        ck("★first_started_utc 는 처음 값을 지킨다(started_utc 는 마지막 셀을 가리킨다)",
           _p["first_started_utc"] == "2026-01-01T10:00:00Z"
           and _p["started_utc"] == "2026-01-01T20:00:00Z")
        ck("authored_by 가 진행표에 남는다", _p["authored_by"] == "main")

        writer_set_cell(camp, cell="c2", outcome="serve_failed", node="main", version=None,
                        model=None, decode_tps=None, measurement_source=None,
                        void_reason="KV 클램프가 로드 첨두를 못 넘김", void_reason_source="엔진 로그",
                        axis_citation="attn 축", next_intent="축 이동 — moe", utc="t2")
        brief2 = resume_brief("w1")
        ck("반증된 축이 재시도 금지 목록에 뜬다",
           "재시도 금지" in brief2 and "KV 클램프가 로드 첨두를 못 넘김" in brief2)
        ck("전 셀 종결이면 pending 없음으로 뜬다", "다음 pending 셀: 없음" in brief2)

        # ★ ACTIVE=_bootstrap 이면 writer 는 no-op 이다(캠페인 밖 평시 서빙 보호).
        ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
        ck("_bootstrap 이면 writer 대상이 없다(no-op 신호)", _writer_target(None) is None)
        ck("_bootstrap resume-brief 는 '캠페인 없음'을 말한다",
           "활성 캠페인이 **없다**" in resume_brief(None))
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")
        ck("★음성대조 없는 인스턴스를 명시하면 거부", _boom(lambda: _writer_target("nope")))

        # ── `_bootstrap` 대기실 정리(2026-09-07 · 사용자 결정) ────────────────────────────
        _bs = CAMPAIGNS / BOOTSTRAP / "relay"
        _bs.mkdir(parents=True, exist_ok=True)
        (_bs / ".gitkeep").write_text("", encoding="utf-8")
        (_bs / "old-ctx.json").write_text("{}", encoding="utf-8")
        (_bs / "old-ctx.reports").mkdir()
        (_bs / "pending_hitl.json").write_text("{}", encoding="utf-8")
        _dry = sweep_bootstrap_relay(apply=False)
        ck("dry-run 은 대기실을 지우지 않는다",
           len(_dry) == 3 and (_bs / "old-ctx.json").is_file())
        _did = sweep_bootstrap_relay(apply=True)
        ck("apply 는 옛 원장·리포트·pending 을 비운다",
           len(_did) == 3 and not (_bs / "old-ctx.json").exists()
           and not (_bs / "old-ctx.reports").exists())
        ck("★.gitkeep 은 남긴다(자리가 사라지면 다음 릴레이가 갈 곳이 없다)",
           (_bs / ".gitkeep").is_file())
        ck("비어 있으면 아무것도 지우지 않는다(멱등)", sweep_bootstrap_relay(apply=True) == [])

        # ── 파생 선언(슬라이스) · 선언된 관측면 (2026-09-08 · plan_26090813 §4.2) ──────────
        def _boom_p(fn):
            try:
                fn(); return False
            except PurgeGateRefusal:
                return True

        _y2 = _read_json(camp / "campaign.yaml")
        _y2["nodes"] = [{"node_id": "main", "role": "main"}, {"node_id": "sub", "role": "sub"}]
        _y2["assignments"] = {"main": [{"cell": "c1"}], "sub": [{"cell": "c9", "mode": "STAY"}]}
        _write_json(camp / "campaign.yaml", _y2)
        _sl = emit_slice(camp, "sub", utc="2026-09-08T00:00:00Z")
        ck("★파생 선언은 그 노드 몫만 담는다(빌딩블럭이 아니라 지시서)",
           list(_sl["assignments"]) == ["sub"] and len(_sl["nodes"]) == 1
           and _sl["self_role"] == "sub" and _sl["authored_by"] == SLICE_AUTHOR
           and _sl["derived_from"] == "main")
        ck("★파생 선언은 통제변인·예산·개정을 그대로 echo 한다(서브가 재저작 ✗)",
           _sl["control_variables"] == _y2["control_variables"]
           and _sl["revisions"] == _y2["revisions"])
        ck("★승인이 없으면 파생 선언의 hint_targets 도 빈 목록이다(운반할 것이 없다)", _sl["hint_targets"] == [])
        ck("★음성대조 선언에 없는 노드는 자를 수 없다",
           _boom(lambda: emit_slice(camp, "ghost", utc="t")))
        _y3 = dict(_y2, assignments={"main": [{"cell": "c1"}]})
        _write_json(camp / "campaign.yaml", _y3)
        ck("★음성대조 배정이 빈 노드의 지시서는 거부(보낼 배정이 없는 지시서는 지시서가 아니다)",
           _boom(lambda: emit_slice(camp, "sub", utc="t")))
        _write_json(camp / "campaign.yaml", _y2)

        # ── 셀 입력 materialize (2026-10-16 · plan_26101622 F1) ─────────────────────────────────
        _mr = Path(tmp) / "mrepo"
        (_mr / "output" / "single" / "configs").mkdir(parents=True)
        (_mr / "output" / "single" / "envs").mkdir(parents=True)
        (_mr / "output" / "single" / "configs" / "c9.yaml").write_text(
            "model: /app/models/X/Y\nmax-model-len: 32768\ngpu-memory-utilization: 0.39\n", encoding="utf-8")
        (_mr / "output" / "single" / "envs" / ".env.c9").write_text("SERVING_MODEL_NAME=y-2b\n", encoding="utf-8")
        (_mr / "output" / "single" / "manifest.yaml").write_text(
            'gpu_model: "NVIDIA GB10"\nmodel_source: managed\nnas_model_path: "/nas/root"\n', encoding="utf-8")
        _mi = Path(tmp) / "meminfo"
        _mi.write_text("MemTotal:       127604828 kB\n", encoding="utf-8")
        _ym = dict(_y2, nodes=[{"node_id": "main", "role": "main", "topology": "single"},
                               {"node_id": "sub", "role": "sub", "topology": "single"}])
        _write_json(camp / "campaign.yaml", _ym)
        _full = {"vram_budget_gb": "47.7", "per_card_vram_gib": "null", "ple_mode": "none", "control_variable": "null",
                 "concurrency_requirement": "null", "typical_request_tokens": "null", "tiktoken_host_path": "null"}
        ck("★음성대조 선언 축이 비면 쓰지 않는다(빈칸 목록을 말한다)",
           _boom(lambda: writer_materialize_cell(camp, cell="c9", repo=_mr, sets={}, utc="2026-10-16T00:00:00Z",
                                                 meminfo=_mi)) and not (camp / "cells" / "c9").exists())
        ck("★음성대조 배정 밖 셀은 만들지 않는다",
           _boom(lambda: writer_materialize_cell(camp, cell="ghost", repo=_mr, sets=_full, utc="t", meminfo=_mi)))
        ck("★음성대조 닫힌 목록 밖 --set 키 거부",
           _boom(lambda: writer_materialize_cell(camp, cell="c9", repo=_mr, sets=dict(_full, max_num_seqs="8"),
                                                 utc="t", meminfo=_mi)))
        _mp = writer_materialize_cell(camp, cell="c9", repo=_mr, sets=_full, utc="2026-10-16T00:00:00Z", meminfo=_mi)
        import yaml as _yaml
        _mc = _yaml.safe_load((camp / "cells" / "c9" / "config.yaml").read_text(encoding="utf-8"))
        ck("파생값은 서빙 yaml·env·manifest·meminfo 에서 온다(손값 ✗)",
           _mc["target_model"]["path"] == "/app/models/X/Y" and _mc["declared_axes"]["context_len"] == 32768
           and _mc["declared_axes"]["tp"] == 1 and _mc["declared_axes"]["kv_cache_dtype"] == "auto"
           and _mc["declared_axes"]["variant"] == "stock" and _mc["target_gpu"]["target_gmu"] == 0.39
           and _mc["target_gpu"]["gpu_model"] == "NVIDIA GB10" and _mc["nas_host_root"] == "/nas/root"
           and _mc["serving"]["served_model_name"] == "y-2b" and _mc["serving"]["config_name"] == "c9"
           and _mc["test_device_total_gib"] == 121.7)
        ck("선언 축은 --set 값 그대로 · 빈칸 0", _mc["vram_budget_gb"] == 47.7 and _mc["declared_axes"]["ple_mode"] == "none"
           and "<<FILL>>" not in (camp / "cells" / "c9" / "config.yaml").read_text(encoding="utf-8"))
        _ml = _read_json(camp / "cells" / "c9" / "lockset.json")
        _ms = _read_json(camp / "cells" / "c9" / "cell.status.json")
        ck("lockset = id·hand-authored 표시 · cell.status = 셀·노드", _ml["id"] == "c9"
           and _ml["provenance"] == "hand-authored" and _ml["max_model_len"] == 32768
           and _ms["cell_id"] == "c9" and _ms["node_id"] == "sub" and len(_mp) == 3)
        ck("★음성대조 덮어쓰기 거부(재작성은 사람이 지우고 다시)",
           _boom(lambda: writer_materialize_cell(camp, cell="c9", repo=_mr, sets=_full, utc="t", meminfo=_mi)))
        shutil.rmtree(camp / "cells" / "c9")
        _write_json(camp / "campaign.yaml", _y2)

        _slp = Path(tmp) / "slice.json"
        _slp.write_text(json.dumps(_sl, ensure_ascii=False), encoding="utf-8")
        _slp2 = Path(tmp) / "slice2.json"
        _slp2.write_text(json.dumps(dict(_sl, id="w2"), ensure_ascii=False), encoding="utf-8")
        scaffold("w2", "docs/plan/ignored.md", apply=True, from_slice=str(_slp2))
        _w2 = _read_json(CAMPAIGNS / "w2" / "campaign.yaml")
        ck("★서브는 파생 선언 하나로 자기 인스턴스를 연다(--from-slice)",
           _w2["self_role"] == "sub" and list(_w2["assignments"]) == ["sub"] and _w2["id"] == "w2")
        ck("★파생으로 열면 plan_ref 는 메인 선언의 것을 지킨다(개설 인자가 덮지 않는다)",
           _w2["plan_ref"] == _y2["plan_ref"])
        ck("★파생 인스턴스의 증거 스냅샷도 빈칸 없이 열린다",
           _read_json(CAMPAIGNS / "w2" / "evidence_pointers.json")["campaign_id"] == "w2")
        ck("★음성대조 id 가 다른 슬라이스로는 열 수 없다(이름이 곧 증거 연결)",
           _boom_p(lambda: scaffold("w3", "p", apply=True, from_slice=str(_slp))))
        # A2A 지시서 본문은 파일을 만들지 않고 stdin으로 한 선언만 나른다.
        _stdin = sys.stdin
        try:
            sys.stdin = __import__("io").StringIO(json.dumps(dict(_sl, id="w4"), ensure_ascii=False))
            scaffold("w4", "docs/plan/ignored.md", apply=True, from_slice="-")
        finally:
            sys.stdin = _stdin
        _w4 = _read_json(CAMPAIGNS / "w4" / "campaign.yaml")
        ck("★stdin 파생 선언도 파일 입력과 같은 sub 인스턴스를 연다",
           _w4["self_role"] == "sub" and _w4["plan_ref"] == _y2["plan_ref"])
        _stdin = sys.stdin
        try:
            sys.stdin = __import__("io").StringIO("{")
            _bad_stdin = _boom_p(lambda: scaffold("w5", "p", apply=True, from_slice="-"))
        finally:
            sys.stdin = _stdin
        ck("★음성대조 파손 stdin 선언은 fail-closed", _bad_stdin)

        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")
        _bp = write_campaign_brief(camp, node="main", utc="2026-09-08T01:00:00Z", repo_root=tmp)
        _b = _read_json(_bp)
        ck("★브리핑은 docs/logs/<node>/ 기계판독 평면에 앉는다(fetch 가 이미 미러하는 경로)",
           str(_bp).replace("\\", "/").endswith("docs/logs/main/campaign_brief.json"))
        ck("★브리핑이 셀·phase·전진시각을 담는다(메인은 이 파일 외에 서브를 읽지 않는다)",
           _b["node_id"] == "main" and _b["cells"]
           and _b["phases"]["serve"]["state"] == "running" and _b["last_utc"])
        ck("★브리핑의 authored_by 는 자기 노드다(P5 가 사후 재저작을 가르는 자리)",
           _b["authored_by"] == "main")

        # ── 회수 편입 (2026-09-08 · plan_26090813 D13) ────────────────────────────────────
        #    증거 경로는 **저장소 상대**여야 하므로 이 블록만 REPO_ROOT 를 픽스처로 옮긴다.
        _saved_root = REPO_ROOT
        REPO_ROOT = Path(tmp)
        _mir = Path(tmp) / "sync_staging" / "sub_docs"
        (_mir / "logs" / "sub").mkdir(parents=True)
        (_mir / "testlog").mkdir(parents=True)
        (_mir / "benchmark").mkdir(parents=True)
        (_mir / "testlog" / "testlog_26090807_서브.md").write_text("판정\n", encoding="utf-8")
        (_mir / "benchmark" / "bench_report_26090807_x.md").write_text("리포트\n", encoding="utf-8")
        (_mir / "benchmark" / "notes.md").write_text("규약 밖\n", encoding="utf-8")
        (_mir / "benchmark" / "bench_report_19990101_historical.md").write_text("과거\n", encoding="utf-8")
        (_mir / "logs" / "old").mkdir(parents=True)
        _write_json(_mir / "logs" / "old" / "campaign_brief.json", {
            "campaign_id": "old-campaign", "node_id": "old", "self_role": "main"})
        _write_json(_mir / "logs" / "sub" / "campaign_brief.json", {
            "schema_version": 1, "campaign_id": "w1", "node_id": "sub", "self_role": "sub",
            "authored_by": "sub", "generated_utc": "2026-09-08T22:00:00Z",
            "cells": [{"cell_id": "c9", "cell_outcome": "measured", "decode_tps_conc1": 7.5,
                       "axis_citation": "tq4 축", "void_reason": None}],
            "phases": {"build": {"state": "done", "proof_ok": True, "proof_source": "docker inspect",
                                 "first_started_utc": "2026-09-08T18:00:00Z",
                                 "ended_utc": "2026-09-08T19:00:00Z"},
                       "serve": {"state": "done", "proof_ok": True, "proof_source": "health 200",
                                 "first_started_utc": "2026-09-08T19:10:00Z",
                                 "ended_utc": "2026-09-08T19:20:00Z"}},
            "journey_tail": [{"utc": "2026-09-08T20:00:00Z", "cell_id": "c9",
                              "next_intent": "d 착수"}],
            "evidence_pointers": [
                {"kind": "testlog", "path": "docs/testlog/testlog_26090807_서브.md",
                 "cell_id": None, "node_id": "sub"},
                {"kind": "bench_report", "path": "docs/benchmark/bench_report_26090807_x.md",
                 "cell_id": "c9", "node_id": "sub"}],
            "last_utc": "2026-09-08T22:00:00Z"})
        # Invalid tail claims must refuse before writing even the earlier valid phase/cell facts.
        _brief_file = _mir / "logs" / "sub" / "campaign_brief.json"
        _brief_good = _read_json(_brief_file)
        def _tree_bytes(root):
            return {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        _before_import = _tree_bytes(camp)
        _bad_claims = [
            ("non-doc relay", {"kind": "relay_summary", "path": "campaigns/w1/relay/ctx.json", "node_id": "sub", "cell_id": "c9"}),
            ("unknown kind", {"kind": "unknown", "path": "docs/testlog/t.md", "node_id": "sub"}),
            ("cross-node", {"kind": "testlog", "path": "docs/testlog/t.md", "node_id": "main"}),
            ("cross-cell", {"kind": "testlog", "path": "docs/testlog/t.md", "node_id": "sub", "cell_id": "c1"}),
            ("escape", {"kind": "testlog", "path": "docs/../outside.md", "node_id": "sub"}),
            ("non-object", "bad-tail"),
        ]
        for _label, _bad in _bad_claims:
            _mut = json.loads(json.dumps(_brief_good))
            _mut["evidence_pointers"].append(_bad)
            _write_json(_brief_file, _mut)
            ck(f"★import malformed tail {_label} → all campaign bytes unchanged",
               _boom(lambda: import_sub_mirror(camp, _mir, utc="2026-09-08T22:30:00Z", unfreeze=True))
               and _tree_bytes(camp) == _before_import)
        _write_json(_brief_file, _brief_good)
        _late_brief = _mir / "logs" / "zzz" / "campaign_brief.json"
        _write_json(_late_brief, _mut)
        ck("★import validates later brief before applying earlier valid brief",
           _boom(lambda: import_sub_mirror(camp, _mir, utc="2026-09-08T22:30:00Z", unfreeze=True))
           and _tree_bytes(camp) == _before_import)
        _late_brief.unlink()

        # A delegated plan_ref remains main authority; a locally authored plan is evidence only.
        _planbase = REPO_ROOT / "campaigns" / "local-plan"
        _plan_decl = {
            "id": "local-plan", "self_role": "sub", "plan_ref": "docs/plan/plan_main_authority.md",
            "nodes": [{"node_id": "sub", "role": "sub", "topology": "single"}],
            "assignments": {"sub": [{"cell": "pc1"}]}, "control_variables": {"topology": "single"},
            "hint_targets": [{"node_id": "sub", "cells": ["pc1"], "approval": {
                "approved_by": "fixture: prior main approval", "approved_utc": "2026-10-27T00:00:00Z",
                "source": "fixture: main declaration slice"}}]}
        _write_json(_planbase / "campaign.yaml", _plan_decl)
        _plan_before = (_planbase / "campaign.yaml").read_bytes()
        _local_plan_rel = "docs/plan/plan_sub_execution.md"
        _local_plan = REPO_ROOT / _local_plan_rel
        _local_plan.parent.mkdir(parents=True, exist_ok=True)
        _local_plan.write_text("Fixture: local execution narrative within the delegated scope.\n", encoding="utf-8")
        writer_add_evidence(_planbase, kind="plan", path_rel=_local_plan_rel,
                            node=None, cell_id="pc1", unfreeze=False)
        _plan_ptrs = _read_json(_planbase / "evidence_pointers.json")["pointers"]
        ck("★local plan writer preserves main plan_ref/approval bytes without copying the main plan",
           (_planbase / "campaign.yaml").read_bytes() == _plan_before
           and not (REPO_ROOT / _plan_decl["plan_ref"]).exists()
           and _plan_ptrs == [{"kind": "plan", "path": _local_plan_rel, "node_id": "sub", "cell_id": "pc1"}])
        _plan_tree = _tree_bytes(_planbase)
        for _label, _path, _cell in (("absent local file", "docs/plan/absent.md", "pc1"),
                                     ("other cell", _local_plan_rel, "other"),
                                     ("directory", "docs/plan", "pc1")):
            ck(f"★local plan {_label} refuses without mutating campaign bytes",
               _boom(lambda: writer_add_evidence(_planbase, kind="plan", path_rel=_path,
                                                 node="sub", cell_id=_cell, unfreeze=False))
               and _tree_bytes(_planbase) == _plan_tree)
        _plan_brief = campaign_brief(_planbase, node="sub", utc="2026-10-27T01:00:00Z")
        ck("★brief carries local plan evidence separately from the unchanged authoritative plan_ref",
           _plan_brief["plan_ref"] == _plan_decl["plan_ref"]
           and _plan_brief["evidence_pointers"] == _plan_ptrs)
        _plan_main = REPO_ROOT / "plan-import" / "local-plan"
        _write_json(_plan_main / "campaign.yaml", dict(_plan_decl, self_role="main", nodes=[
            {"node_id": "main", "role": "main", "topology": "single"}, *_plan_decl["nodes"]]))
        _main_plan_before = (_plan_main / "campaign.yaml").read_bytes()
        _plan_mirror = REPO_ROOT / "sync_staging" / "plan_docs"
        _write_json(_plan_mirror / "logs/sub/campaign_brief.json", _plan_brief)
        _mirrored_plan = _plan_mirror / _local_plan_rel.removeprefix("docs/")
        _mirrored_plan.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(_local_plan, _mirrored_plan)
        (_mirrored_plan.parent / "plan_unclaimed.md").write_text("Unclaimed fixture.\n", encoding="utf-8")
        _plan_wrote, _plan_warn = import_sub_mirror(_plan_main, _plan_mirror, utc="2026-10-27T01:05:00Z")
        ck("★writer→brief→import preserves cell/node-scoped plan membership without adopting unclaimed plans",
           bool(_plan_wrote) and not _plan_warn
           and _read_json(_plan_main / "evidence_pointers.json")["pointers"] == [
               dict(_plan_ptrs[0], path=_rel(_mirrored_plan))]
           and (_plan_main / "campaign.yaml").read_bytes() == _main_plan_before)
        _mirrored_plan.unlink()
        _plan_import_before = _tree_bytes(_plan_main)
        _plan_wrote, _plan_warn = import_sub_mirror(_plan_main, _plan_mirror, utc="2026-10-27T01:10:00Z")
        ck("★missing mirrored plan remains a reported evidence gap, never a replacement approval",
           not _plan_wrote and len(_plan_warn) == 1 and "증거 미등재" in _plan_warn[0]
           and _tree_bytes(_plan_main) == _plan_import_before)

        # The sub writer refuses a local control ledger as upward evidence, without deleting it.
        _subbase = REPO_ROOT / "campaigns" / "sub-evidence"
        _write_json(_subbase / "campaign.yaml", {
            "self_role": "sub", "nodes": [{"node_id": "sub", "role": "sub", "topology": "single"}],
            "assignments": {"sub": [{"cell": "sc1"}]}, "control_variables": {"topology": "single"}})
        _relay = _subbase / "relay" / "ctx.json"
        _write_json(_relay, {"fixture": "local relay retained"})
        _relay_rel = _rel(_relay)
        _summary = REPO_ROOT / "docs" / "testlog" / "testlog_fixture_relay.md"
        _summary.parent.mkdir(parents=True, exist_ok=True)
        _summary.write_text("fixture: docs relay summary\n", encoding="utf-8")
        _summary_rel = _rel(_summary)
        ck("★sub writer refuses campaigns relay pointer before creating evidence state",
           _boom(lambda: writer_add_evidence(_subbase, kind="relay_summary", path_rel=_relay_rel,
                                             node="sub", cell_id="sc1", unfreeze=False))
           and not (_subbase / "evidence_pointers.json").exists())
        _summary_link = _summary.parent / "testlog_fixture_link.md"
        _summary_link.symlink_to(_summary)
        ck("★sub writer rejects symlink evidence even inside docs",
           _boom(lambda: writer_add_evidence(_subbase, kind="testlog", path_rel=_rel(_summary_link),
                                             node="sub", cell_id="sc1", unfreeze=False))
           and not (_subbase / "evidence_pointers.json").exists())
        _rawdir = REPO_ROOT / "docs" / "simlog" / "fixture-vault"
        _rawdir.mkdir(parents=True)
        writer_add_evidence(_subbase, kind="simlog", path_rel=_rel(_rawdir),
                            node="sub", cell_id="sc1", unfreeze=False)
        ck("★sub writer preserves simlog-directory evidence contract",
           _read_json(_subbase / "evidence_pointers.json")["pointers"][0]["path"] == _rel(_rawdir))
        _write_json(_subbase / "evidence_pointers.json", {"pointers": [
            {"kind": "relay_summary", "path": _relay_rel, "node_id": "sub", "cell_id": "sc1"}]})
        _before_correction = _tree_bytes(_subbase)
        def _correct(**kw):
            args = dict(kind="relay_summary", path_from=_relay_rel, path_to=_summary_rel,
                        reason="Replace erroneous control-ledger claim with upward docs summary",
                        utc="2026-09-08T22:40:00Z", unfreeze=False, node="sub", cell_id="sc1")
            args.update(kw)
            return writer_relocate_evidence(_subbase, **args)
        ck("★default relocate still refuses an existing source and changes nothing",
           _boom(_correct) and _tree_bytes(_subbase) == _before_correction)
        for _label, _kw in (("reason", {"reason": " "}), ("utc", {"utc": None}),
                            ("node", {"node": "main"}), ("cell", {"cell_id": "other"}),
                            ("destination", {"path_to": _relay_rel})):
            ck(f"★reference correction requires valid {_label} and preserves original bytes",
               _boom(lambda: _correct(correct_reference=True, **_kw))
               and _tree_bytes(_subbase) == _before_correction)
        _source_bytes = _relay.read_bytes()
        _correct(correct_reference=True)
        _fixed_ptr = _read_json(_subbase / "evidence_pointers.json")["pointers"][0]
        ck("★reference correction preserves relay file and node/cell, records source/reason/UTC/mode",
           _relay.read_bytes() == _source_bytes and _fixed_ptr["path"] == _summary_rel
           and _fixed_ptr["node_id"] == "sub" and _fixed_ptr["cell_id"] == "sc1"
           and _fixed_ptr["relocated"][0]["from"] == _relay_rel
           and _fixed_ptr["relocated"][0]["mode"] == "reference-correction"
           and _fixed_ptr["relocated"][0]["source_preserved"] is True
           and bool(_fixed_ptr["relocated"][0]["reason"]) and _fixed_ptr["relocated"][0]["utc"])
        _sweep_rel = "output/single/benchlog/sweep_cfg-new"
        _idxpath = REPO_ROOT / _sweep_rel / "sweep_index.json"
        _write_json(_idxpath, {"generated_utc": "2026-09-08T22:45:00Z", "meta": {"config_name": "cfg-new"}})
        _sweep_state = _subbase / "sweeps" / "s.json"
        _sweep_rec = {"cell_key": "sc1", "config": "cfg-new", "cell_outcome": "measured",
                      "sweep_dir": _sweep_rel, "sweep_generated_utc": "2026-09-08T22:45:00Z"}
        _write_json(_sweep_state, {"cells": [_sweep_rec]})
        _outptr = _sweep_output(_subbase, _sweep_state, "sc1", "sub")
        ck("★official sweep output binding preserves actual config even when config != cell",
           _outptr["sweep_dir"] == _sweep_rel and _outptr["sweep_config"] == "cfg-new"
           and _outptr["sweep_generated_utc"] == "2026-09-08T22:45:00Z")
        def _write_sweep_cell():
            return writer_set_cell(_subbase, cell="sc1", outcome="measured", node="sub",
                version=None, model=None, decode_tps="6", measurement_source="fixture: measured producer row",
                void_reason=None, void_reason_source=None, axis_citation=None, next_intent=None,
                utc="2026-09-08T22:45:00Z", sweep_state=str(_sweep_state))
        _write_sweep_cell()
        ck("★cell writer carries verified fresh sweep pointer from producer without manual setter",
           _read_json(_subbase / "cells/sc1/cell.status.json")["measurement"]["sweep_dir"] == _sweep_rel)
        _before_bad_sweep = _tree_bytes(_subbase / "cells")
        _nested_sweep = _sweep_rel + "/sweep_nested"
        _write_json(REPO_ROOT / _nested_sweep / "sweep_index.json", _read_json(_idxpath))
        for _field, _value in (("sweep_dir", "output/multi/benchlog/sweep_cfg-new"),
                               ("sweep_dir", "output/single/benchlog/../escape"),
                               ("sweep_dir", _nested_sweep),
                               ("sweep_generated_utc", "stale"), ("config", "other")):
            _write_json(_sweep_state, {"cells": [dict(_sweep_rec, **{_field: _value})]})
            ck(f"★sweep output rejects {_field}={_value}",
               _boom(_write_sweep_cell) and _tree_bytes(_subbase / "cells") == _before_bad_sweep)
        _write_json(_sweep_state, {"cells": [{"cell_key": "sc1"}]})
        ck("★legacy sweep without output pointer remains supported",
           _sweep_output(_subbase, _sweep_state, "sc1", "sub") is None)
        _write_sweep_cell()
        ck("★new legacy sweep does not retain previous run's explicit output pointer",
           "sweep_dir" not in _read_json(_subbase / "cells/sc1/cell.status.json")["measurement"])
        _write_json(_sweep_state, {"cells": [_sweep_rec]})
        _index_copy = _idxpath.with_name("index-fixture.json")
        _idxpath.rename(_index_copy)
        _idxpath.symlink_to(_index_copy)
        ck("★sweep writer refuses symlink index before reading it",
           _boom(_write_sweep_cell))
        _n0 = len(read_journey(camp))
        _w0, _warn0 = import_sub_mirror(camp, _mir, utc="2026-09-08T23:00:00Z")
        ck("★동결된 스냅샷을 만나도 진행표는 적히고 증거만 결손 기재된다(차단 ✗ · D17)",
           _warn0 and all("증거 미등재" in x for x in _warn0)
           and (camp / "phases" / "sub" / "build.status.json").is_file())
        _w, _warn = import_sub_mirror(camp, _mir, utc="2026-09-08T23:00:00Z", unfreeze=True)
        _sb = _read_json(camp / "phases" / "sub" / "build.status.json")
        ck("★회수 편입이 서브 phase 를 서브 저작으로 남긴다(사후 손저작 대체)",
           _sb["authored_by"] == "sub" and _sb["state"] == "done"
           and _sb["first_started_utc"] == "2026-09-08T18:00:00Z")
        ck("★phase 시각이 서로 다르게 보존된다(P5 동일초 판정이 성립하려면 필요)",
           _read_json(camp / "phases" / "sub" / "serve.status.json")["ended_utc"]
           != _sb["ended_utc"])
        _sc = _read_json(camp / "cells" / "c9" / "cell.status.json")
        ck("★회수 편입이 셀 결과·측정·노드를 옮긴다",
           _sc["cell_outcome"] == "measured" and _sc["measurement"]["decode_tps_conc1"] == 7.5
           and _sc["node_id"] == "sub")
        _scp = _sc.get("provenance") or {}
        ck("★회수 편입된 서브 셀은 lockset '부재' 가 아니라 **관측 불가**로 기재된다(P6 와 같은 판정 · "
           "메인에게 hand-authored 처방 ✗)",
           _scp.get("observability") == "not_observable" and _scp.get("lockset_gap") is None
           and "hand-authored" not in json.dumps(_sc, ensure_ascii=False))
        _ptrs = _read_json(camp / "evidence_pointers.json")["pointers"]
        ck("★회수 문서가 **노드 태그와 함께** 증거로 등재된다(P2 공허 통과 방지)",
           any(x["kind"] == "testlog" and x["node_id"] == "sub" for x in _ptrs)
           and any(x["kind"] == "bench_report" and x["node_id"] == "sub" for x in _ptrs))
        ck("★규약 밖·과거 mirror 문서는 claim 없이 evidence가 되지 않는다",
           not any("notes.md" in str(x.get("path")) or "historical" in str(x.get("path"))
                   for x in _ptrs))
        ck("★다른 campaign brief는 shared mirror 역사로 무시한다",
           all(x.get("node_id") != "old" for x in _ptrs))
        ck("여정 한 줄이 회수와 함께 편입된다", len(read_journey(camp)) == _n0 + 1)
        import_sub_mirror(camp, _mir, utc="2026-09-08T23:30:00Z", unfreeze=True)
        ck("★재편입은 여정을 늘리지 않는다(멱등)", len(read_journey(camp)) == _n0 + 1)
        _write_json(_mir / "logs" / "sub" / "campaign_brief.json",
                    {"schema_version": 1, "campaign_id": "다른캠페인", "node_id": "sub"})
        ck("★음성대조 다른 캠페인의 브리핑은 편입 거부(남의 사실을 주장하지 않는다)",
           _boom(lambda: import_sub_mirror(camp, _mir, utc="t")))
        (_mir / "logs" / "sub" / "campaign_brief.json").unlink()
        ck("★음성대조 브리핑 부재는 소리낸다('안 돌았다'와 '관측면이 안 왔다'는 다르다)",
           _boom(lambda: import_sub_mirror(camp, _mir, utc="t")))

        # ── 사후 수리 (2026-09-08 · plan_26090813 §4.4 · D17) ─────────────────────────────
        (Path(tmp) / "docs" / "benchmark").mkdir(parents=True, exist_ok=True)
        _write_json(Path(tmp) / "docs" / "benchmark" / "sweep_map_x.json", {"cells": [
            {"cell_key": "c3", "cell_outcome": "measured", "concurrency_vector": {"1": 11.5}}]})
        writer_set_cell(camp, cell="c4", outcome="pending", node="main", version=None, model=None,
                        decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None, utc="t")
        _rep, _gaps = backfill_from_docs(camp, utc="2026-09-09T00:00:00Z")
        _c3 = _read_json(camp / "cells" / "c3" / "cell.status.json")
        ck("★결손 셀의 측정을 sweep map 에서 사후 수리한다(hint 발행 직전 · 차단 ✗)",
           _c3["measurement"]["decode_tps_conc1"] == 11.5
           and _c3["measurement"]["source"].startswith("backfill:"))
        ck("★수리해도 없는 것은 **이름을 부른다**(조용한 결손 금지)",
           any("c4" in g for g in _gaps))
        ck("★수리는 이미 채워진 값을 덮지 않는다",
           _read_json(camp / "cells" / "c9" / "cell.status.json")
           ["measurement"]["decode_tps_conc1"] == 7.5)
        REPO_ROOT = _saved_root

        # ── 그라운딩 안전불변식 (2026-09-08 · policy:LIBRARY_GROUNDING_FAIL_CLOSED) ────────
        ck("★기록이 없으면 백스톱이 막는다(C2 fail-closed)",
           any("그라운딩 기록이 없다" in r for r in grounding_reasons(camp)))
        ck("질의어를 통제변인에서 파생한다(손저작 ✗)",
           "m1" in grounding_terms({"control_variables": {"model": "m1", "_note": "x"}})
           and not grounding_terms({"control_variables": {"model": FILL}}))

        def _put_grounding(status, reason=None):
            _write_json(camp / GROUNDING_DIR / "20260908T000000Z.json", {
                "schema_version": 1, "campaign_id": "w1", "asked_utc": "2026-09-08T00:00:00Z",
                "request": {"query": {"terms": ["m1"]}},
                "export": {"resolution": {"status": status, "librarian": "wiki-desk",
                                          **({"reason": reason} if reason else {})},
                           "references": []},
                "attestation": {"status": status,
                                "grounding_gap": (None if status == "resolved"
                                                  else {"status": status, "asked_utc": "t"})}})

        _put_grounding("resolved")
        ck("기록이 있으면 통과", not grounding_reasons(camp))
        _put_grounding("unresolved", "정직한 공백")
        ck("★사서가 못 찾은 것은 **통과**다(공백은 차단이 아니다 · D5)", not grounding_reasons(camp))
        _put_grounding("refused")
        ck("★사서가 **거절**한 것은 차단이다(공백과 거절은 다른 사실)",
           any("거절" in r for r in grounding_reasons(camp)))
        _put_grounding("unresolved", "정직한 공백")
        writer_set_cell(camp, cell="c1", outcome="measured", node="main", version=None,
                        model=None, decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None, utc="t5")
        _cg = _read_json(camp / "cells" / "c1" / "cell.status.json")["grounding"]
        ck("★C3 산출물이 자기 그라운딩을 스스로 밝힌다(status·source·gap)",
           _cg["status"] == "unresolved" and "grounding/" in _cg["source"] and _cg["gap"])
        import shutil as _sh
        _sh.rmtree(camp / GROUNDING_DIR)
        writer_set_cell(camp, cell="c1", outcome="measured", node="main", version=None,
                        model=None, decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None, utc="t6")
        ck("★기록이 없으면 산출물이 `absent` 라고 말한다(부재를 침묵하지 않는다)",
           _read_json(camp / "cells" / "c1" / "cell.status.json")["grounding"]["status"] == "absent")

        # ── 어휘 tripwire (2026-09-08 라이브 교정) ────────────────────────────────────────
        #    분류기가 내는 값을 writer 가 모르면 셀 기록이 통째로 사라진다. 실제로 그랬다.
        _cc = REPO_ROOT.parent if False else None
        _cc_path = (Path(__file__).resolve().parents[2] / "adversarial-benchmark" / "scripts"
                    / "classify_cell.py")
        if _cc_path.is_file():
            _src = _cc_path.read_text(encoding="utf-8")
            _vocab = set(re.findall(r'^OUTCOME_[A-Z_]+\s*=\s*"([a-z_]+)"', _src, re.M))
            ck("★분류기 어휘가 writer 어휘의 부분집합이다(갈라지면 셀 기록이 사라진다)",
               bool(_vocab) and _vocab <= set(CELL_OUTCOMES))
        else:
            ck("분류기 경로를 찾지 못했다(어휘 대조 불가 — 부재를 통과로 접지 않는다)", False)

        # ── R5 이미지 축 · R6 escalation 후보 (2026-09-11 · plan_26091108) ──────────────
        (camp / "sweeps").mkdir(parents=True, exist_ok=True)
        _sw = camp / "sweeps" / "img.json"
        _sw.write_text(json.dumps({"cells": [{"cell_key": "ci1", "cell_outcome": "measured",
            "coordinates": {"image_digest": "sha256:aaa", "image_tag": "t:1"},
            "concurrency_vector": {"1": 9.5}}]}), encoding="utf-8")
        writer_set_cell(camp, cell="ci1", outcome="measured", node="main", version=None,
                        model=None, decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None,
                        utc="2026-09-11T00:00:00Z", sweep_state=str(_sw))
        _ci1 = _read_json(camp / "cells" / "ci1" / "cell.status.json") or {}
        ck("★R5 이미지 좌표를 sweep 에서 파생한다(호출부가 나르지 않는다)",
           (_ci1.get("image") or {}).get("digest") == "sha256:aaa"
           and "sweep:" in ((_ci1.get("image") or {}).get("source") or ""))
        writer_set_cell(camp, cell="ci2", outcome="measured", node="main", version=None,
                        model=None, decode_tps=None, measurement_source=None, void_reason=None,
                        void_reason_source=None, axis_citation=None, next_intent=None,
                        utc="2026-09-11T00:00:00Z")
        _ci2 = _read_json(camp / "cells" / "ci2" / "cell.status.json") or {}
        ck("★R5 digest 가 없으면 **결손을 기재**한다(차단 ✗ — 진행을 막는 자리는 셋뿐)",
           (_ci2.get("image") or {}).get("digest") is None
           and "digest" in ((_ci2.get("image") or {}).get("gap") or ""))
        ck("★R5 음성대조: 파생값과 다른 digest 를 선언하면 거부한다(결손이 아니라 거짓)",
           _boom(lambda: writer_set_cell(camp, cell="ci1", outcome="measured", node="main",
                 version=None, model=None, decode_tps=None, measurement_source=None,
                 void_reason=None, void_reason_source=None, axis_citation=None,
                 next_intent=None, utc="2026-09-11T00:00:00Z", sweep_state=str(_sw),
                 image_digest="sha256:bbb")))

        _kill = ['{"ts":"2026-09-11T00:00:30Z","kind":"watchdog_kill_ack","source":"unit:x"}']
        _evp = camp / "ev.jsonl"
        _evp.write_text("\n".join(_kill), encoding="utf-8")
        v_host = judge_escalation_predicate(["serve_init_immediate_death"], events=[str(_evp)],
                                            started_utc="2026-09-11T00:00:00Z",
                                            ended_utc="2026-09-11T00:01:00Z")
        writer_append_escalation(camp, {"utc": "2026-09-11T00:01:00Z", "cell_id": "ehost",
                                        "predicate_1": v_host["predicate_1"],
                                        "host_plane_ignored": v_host["host_plane_ignored"],
                                        "model_plane_refuted": v_host["model_plane_refuted"]})
        v_std = judge_escalation_predicate(["config_arch_unsupported"])
        writer_append_escalation(camp, {"utc": "2026-09-11T00:02:00Z", "cell_id": "estand",
                                        "predicate_1": v_std["predicate_1"],
                                        "host_plane_ignored": v_std["host_plane_ignored"],
                                        "model_plane_refuted": v_std["model_plane_refuted"]})
        _sum = escalation_summary(camp)
        ck("★R6 사살로 죽은 셀은 후보에 남되 술어 ① 이 서지 않는다(예산 문제와 빌드 문제를 가른다)",
           _sum["cells_host_plane"] == ["ehost"] and _sum["predicate_1_standing"] == 1)
        ck("★R6 모델 평면 증상은 ① 이 서고 종결 HITL 을 요구한다",
           _sum["cells_standing"] == ["estand"] and _sum["hitl_required"] is True)
        ck("★R6 음성대조: 후보가 하나도 없으면 HITL 을 요구하지 않는다(과잉차단 ✗)",
           escalation_summary(camp / "___none")["hitl_required"] is False)
        _bs = (Path(__file__).resolve().parents[2] / "adversarial-benchmark" / "scripts"
               / "broad_search.sh")
        ck("★R6 producer 배선 앵커: 반증 셀에서 후보를 쌓는 호출부가 실재한다",
           _bs.is_file() and "--escalation-add" in _bs.read_text(encoding="utf-8"))
        ck("★R10 셀 키 어휘가 갈라져 노드를 못 구하면 **소리를 낸다**(종전엔 조용히 건너뛰었다)",
           _bs.is_file() and "bench 진행표·브리핑 미기록" in _bs.read_text(encoding="utf-8"))

        # ── 셀 출처 precheck · 불일치 기재 (2026-09-14 · plan_26091407 §4.0) ──────────────────
        #    precheck 는 표시 부재·무효만 막고(hand-authored 통과), writer 는 드리프트를 **기재만** 한다.
        import contextlib as _ctx
        import io as _io
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")
        (camp / "cells" / "c1").mkdir(parents=True, exist_ok=True)
        _lk = camp / "cells" / "c1" / "lockset.json"
        _lk.unlink(missing_ok=True)
        _v = lockset_precheck("c1")
        ck("★precheck: lockset 부재는 refused 이고 사유가 explorer 소관을 말한다",
           _v["status"] == "refused" and "explorer 소관" in (_v["reason"] or ""))
        _write_json(_lk, {"id": "c1", "provenance": "hand-authored"})
        _v = lockset_precheck("c1")
        ck("★precheck 음성대조: hand-authored 는 통과한다(손작성은 금지가 아니라 표시 대상)",
           _v["status"] == "passed" and _v["provenance"] == "hand-authored" and _v["reason"] is None)
        _write_json(_lk, {"id": "c1", "provenance": "explorer"})
        ck("★precheck: 목록 밖 provenance 는 refused", lockset_precheck("c1")["status"] == "refused")
        _write_json(_lk, dict(_read_json(TEMPLATE / "cells" / "_cell" / "lockset.json"), id="c1"))
        ck("★precheck: 뼈대 복사본(<<FILL>>)은 출처를 말하지 않은 것이다",
           lockset_precheck("c1")["status"] == "refused")
        _vz = lockset_precheck("zz-not-assigned")
        ck("★precheck: 배정에 없는 셀 키는 어휘 갈라짐 가능성까지 말한다(R10 계열)",
           _vz["status"] == "refused" and "assignments 에도 없다" in (_vz["reason"] or ""))

        def _cli(*argv):
            _o, _e = _io.StringIO(), _io.StringIO()
            with _ctx.redirect_stdout(_o), _ctx.redirect_stderr(_e):
                rc = main(list(argv))
            return rc, _o.getvalue(), _e.getvalue()

        _rc, _out, _err = _cli("--lockset-precheck", "--cell", "c1")
        ck("★CLI: refused 는 rc 2 이고 판정 JSON 은 stdout · 사유는 stderr",
           _rc == 2 and json.loads(_out)["status"] == "refused" and "거부" in _err)
        _write_json(_lk, {"id": "c1", "provenance": "explorer-phase2"})
        _rc, _out, _err = _cli("--lockset-precheck", "--cell", "c1")
        ck("★CLI 음성대조: explorer-phase2 는 rc 0", _rc == 0 and json.loads(_out)["status"] == "passed")
        _rc, _out, _err = _cli("--lockset-precheck")
        ck("★CLI: --cell 없는 precheck 는 거부(rc≠0 — 셀 없는 출처는 없다)", _rc != 0)
        ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
        _rc, _out, _err = _cli("--lockset-precheck", "--cell", "c1")
        ck("★_bootstrap 은 not_applicable 이고 rc 0 이되 **이름으로 말한다**(조용한 우회 ✗)",
           _rc == 0 and json.loads(_out)["status"] == "not_applicable" and "대상 아님" in _err)
        ACTIVE_POINTER.write_text("camp-typo\n", encoding="utf-8")
        _vt = lockset_precheck("c1")
        ck("★무효 포인터(오타·purge 잔존)는 사유에 **실제 포인터 내용**을 싣는다('명시 _bootstrap' 과 구분)",
           _vt["status"] == "not_applicable" and "camp-typo" in (_vt["reason"] or "")
           and "무효" in (_vt["reason"] or ""))
        ck("★--campaign-id 로 캠페인을 주면 포인터와 무관하게 그 캠페인 셀을 판정한다(포인터 유실 ≠ 게이트 개방)",
           lockset_precheck("c1", "w1")["status"] == "passed")
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")

        (camp / "cells" / "cm").mkdir(parents=True, exist_ok=True)
        (camp / "cells" / "cm" / "config.yaml").write_text(
            "target_gpu:\n  gpu_model: GB10\n  target_gmu: 0.85   # 선언\n", encoding="utf-8")
        _write_json(camp / "cells" / "cm" / "lockset.json",
                    {"id": "cm", "provenance": "hand-authored", "gmu_source": "hand"})
        _swp = camp / "sweeps" / "prov.json"

        def _sweep_gmu(val):
            _swp.write_text(json.dumps({"cells": [{"cell_key": "cm", "cell_outcome": "measured",
                "coordinates": {"gpu_memory_utilization": val, "image_digest": "sha256:ccc"},
                "concurrency_vector": {"1": 5.0}}]}), encoding="utf-8")

        def _set_cm(sweep):
            writer_set_cell(camp, cell="cm", outcome="measured", node="main", version=None,
                            model=None, decode_tps=None, measurement_source=None, void_reason=None,
                            void_reason_source=None, axis_citation=None, next_intent=None,
                            utc="2026-09-14T00:00:00Z", sweep_state=sweep)
            return _read_json(camp / "cells" / "cm" / "cell.status.json") or {}

        _sweep_gmu("0.80")
        _cm = _set_cm(str(_swp))
        _mm = _cm.get("provenance_mismatch") or []
        ck("★불일치 기재: 서빙 yaml gmu 0.80 ≠ 선언 target_gmu 0.85 가 cell.status 에 남는다(F1 재현)",
           len(_mm) == 1 and _mm[0]["knob"] == "gpu_memory_utilization"
           and _mm[0]["declared"] == 0.85 and _mm[0]["observed"] == 0.8
           and "target_gpu.target_gmu" in _mm[0]["declared_source"]
           and "coordinates.gpu_memory_utilization" in _mm[0]["observed_source"]
           and _mm[0]["lockset_gmu_source"] == "hand")
        ck("★불일치는 기재일 뿐 셀 결과를 바꾸지 않는다(차단 ✗)", _cm.get("cell_outcome") == "measured")
        ck("셀 출처 표시가 lockset 에서 옮겨진다(writer 가 직접 읽는다)",
           (_cm.get("provenance") or {}).get("lockset") == "hand-authored"
           and (_cm.get("provenance") or {}).get("lockset_gap") is None)
        ck("★lockset 에 잠기지 않은 파생 노브는 '일치' 가 아니라 대조 불가로 이름이 남는다(안 쟀다 ≠ 일치했다)",
           (_cm.get("provenance") or {}).get("pending_knobs") == []
           and (_cm.get("provenance") or {}).get("compared") == ["gpu_memory_utilization"]
           and sum(1 for g in (_cm.get("provenance") or {}).get("compare_gaps") or []
                   if g.startswith(("batch 대조 불가", "kv_cache_memory_bytes 대조 불가"))) == 2)
        _cm = _set_cm(None)
        ck("★sweep 없이 다시 적어도 직전 불일치 기재를 지우지 않는다(부재가 관측을 덮지 않는다)",
           len(_cm.get("provenance_mismatch") or []) == 1)
        _sweep_gmu("0.85")
        _cm = _set_cm(str(_swp))
        ck("★불일치 음성대조: 같으면 기재 0 건이고 대조했다는 사실은 남는다",
           _cm.get("provenance_mismatch") == []
           and (_cm.get("provenance") or {}).get("compared") == ["gpu_memory_utilization"])
        _sweep_gmu("NA")
        _cm = _set_cm(str(_swp))
        ck("★서빙 gmu 가 없으면 불일치가 아니라 **대조 불가**로 기재한다(일치로 접지 않는다)",
           _cm.get("provenance_mismatch") == [] and (_cm.get("provenance") or {}).get("compared") == []
           and any("대조 불가" in g for g in (_cm.get("provenance") or {}).get("compare_gaps") or []))
        _sweep_gmu("0.80")
        _set_cm(str(_swp))
        _sweep_gmu("NA")
        _cm = _set_cm(str(_swp))
        ck("★불일치 기재 직후 대조 불가(sweep 좌표 NA)가 와도 직전 불일치를 지우지 않는다(부재 ≠ 일치)",
           len(_cm.get("provenance_mismatch") or []) == 1
           and (_cm.get("provenance_mismatch") or [{}])[0].get("observed") == 0.8
           and any("대조 불가" in g for g in (_cm.get("provenance") or {}).get("compare_gaps") or []))
        _sweep_gmu("0.85")
        _cm = _set_cm(str(_swp))
        ck("★다시 대조해 일치하면 그 노브의 기재만 교체된다(0 건)", _cm.get("provenance_mismatch") == [])
        # ── 단계 ② 슬롯: explorer 파생 노브(batch·KV 클램프) — lockset ↔ 서빙 실물(sweep) 대조 ──────────────
        _write_json(camp / "cells" / "cm" / "lockset.json",
                    {"id": "cm", "provenance": "explorer-phase2", "gmu_source": "target_gmu",
                     "batch": 24, "batch_source": "kv-fit-measured",
                     "kv_cache_memory_bytes": 21474836480, "kv_source": "measured-clamp"})

        def _sweep_knobs(seqs, kvb):
            _swp.write_text(json.dumps({"cells": [{"cell_key": "cm", "cell_outcome": "measured",
                "coordinates": {"gpu_memory_utilization": "0.85", "max_num_seqs": seqs,
                                "image_digest": "sha256:ccc"},
                "capacity": {"kv_cache_memory_bytes": kvb},
                "concurrency_vector": {"1": 5.0}}]}), encoding="utf-8")

        _sweep_knobs("8", "21474836480")
        _cm = _set_cm(str(_swp))
        _mm = _cm.get("provenance_mismatch") or []
        ck("★파생 노브 불일치 기재: 서빙 max-num-seqs 8(손레버) ≠ lockset batch 24 — 출처·어휘 칸과 함께(F1 형태)",
           len(_mm) == 1 and _mm[0]["knob"] == "batch" and _mm[0]["declared"] == 24 and _mm[0]["observed"] == 8
           and _mm[0]["declared_source"].endswith("lockset.json#batch")
           and "coordinates.max_num_seqs" in _mm[0]["observed_source"]
           and _mm[0]["lockset_batch_source"] == "kv-fit-measured")
        ck("★파생 노브 대조 목록: gmu·batch·KV 클램프 셋 다 대조했고 대조 불가 0 · pending 0",
           (_cm.get("provenance") or {}).get("compared")
           == ["gpu_memory_utilization", "batch", "kv_cache_memory_bytes"]
           and (_cm.get("provenance") or {}).get("compare_gaps") == []
           and (_cm.get("provenance") or {}).get("pending_knobs") == [])
        ck("★파생 노브 불일치도 기재일 뿐 셀 결과를 바꾸지 않는다(차단 ✗)", _cm.get("cell_outcome") == "measured")
        _sweep_knobs("NA", "21474836480")
        _cm = _set_cm(str(_swp))
        ck("★서빙 yaml 에 max-num-seqs 가 없으면(NA) 불일치가 아니라 대조 불가 — 직전 batch 기재는 유지된다(부재 ≠ 일치)",
           any(g.startswith("batch 대조 불가") for g in (_cm.get("provenance") or {}).get("compare_gaps") or [])
           and "batch" not in ((_cm.get("provenance") or {}).get("compared") or [])
           and [m["knob"] for m in _cm.get("provenance_mismatch") or []] == ["batch"])
        _sweep_knobs("24", "10737418240")
        _cm = _set_cm(str(_swp))
        _mm = _cm.get("provenance_mismatch") or []
        ck("★KV 클램프 불일치 기재(capacity.kv_cache_memory_bytes) · batch 가 일치로 돌아오면 그 기재만 사라진다",
           len(_mm) == 1 and _mm[0]["knob"] == "kv_cache_memory_bytes" and _mm[0]["observed"] == 10737418240
           and _mm[0]["lockset_kv_source"] == "measured-clamp")
        _sweep_knobs("24", "21474836480")
        _cm = _set_cm(str(_swp))
        ck("★파생 노브 음성대조: 셋 다 같으면 기재 0 건",
           _cm.get("provenance_mismatch") == [] and len((_cm.get("provenance") or {}).get("compared") or []) == 3)
        ck("★각인 여부 기재: explorer-phase2 인데 trial_provenance 가 없으면 self-declared(기계 각인과 가른다)",
           (_cm.get("provenance") or {}).get("lockset_stamp") == "self-declared"
           and (_cm.get("provenance") or {}).get("lockset_trial_provenance") is None)
        _write_json(camp / "cells" / "cm" / "lockset.json",
                    {"id": "cm", "provenance": "explorer-phase2", "gmu_source": "target_gmu",
                     "batch": 24, "batch_source": "kv-fit-measured", "trial_provenance": "mock",
                     "kv_cache_memory_bytes": 21474836480, "kv_source": "measured-clamp"})
        _cm = _set_cm(str(_swp))
        ck("★각인 여부 기재: explorer 수렴 각인(trial_provenance=mock)은 machine · mock 이 cell.status 까지 옮겨진다",
           (_cm.get("provenance") or {}).get("lockset_stamp") == "machine"
           and (_cm.get("provenance") or {}).get("lockset_trial_provenance") == "mock")
        # lockset 은 비었는데(null) 서빙 yaml 이 손값을 싣는 F1 형태 — 대조 불가가 아니라 불일치(undetermined)다.
        _write_json(camp / "cells" / "cm" / "lockset.json",
                    {"id": "cm", "provenance": "hand-authored", "gmu_source": "target_gmu",
                     "batch": None, "kv_cache_memory_bytes": 21474836480, "kv_source": "hand"})
        _sweep_knobs("8", "21474836480")
        _cm = _set_cm(str(_swp))
        _mm = _cm.get("provenance_mismatch") or []
        ck("★lockset batch=null ∧ 서빙 max-num-seqs 8 → 불일치 기재(declared=null · undetermined · F1 형태)",
           len(_mm) == 1 and _mm[0]["knob"] == "batch" and _mm[0]["declared"] is None
           and _mm[0]["declared_state"] == "undetermined" and _mm[0]["observed"] == 8
           and "batch" in ((_cm.get("provenance") or {}).get("compared") or [])
           and (_cm.get("provenance") or {}).get("lockset_stamp") == "hand")
        _sweep_knobs("NA", "21474836480")
        _cm = _set_cm(str(_swp))
        ck("★음성대조: lockset batch=null ∧ 서빙 max-num-seqs 없음 → 대조 불가(양쪽 부재는 불일치가 아니다) · 직전 기재 유지",
           any(g.startswith("batch 대조 불가") and "lockset batch 도" in g
               for g in (_cm.get("provenance") or {}).get("compare_gaps") or [])
           and [m["knob"] for m in _cm.get("provenance_mismatch") or []] == ["batch"])
        # 손레버 ↔ explorer 재계산 대조(plan §4.0 Q1).
        _write_json(camp / "cells" / "cm" / "lockset.json",
                    {"id": "cm", "provenance": "explorer-phase2", "gmu_source": "target_gmu", "trial_provenance": "mock",
                     "batch": 8, "batch_source": "hand-lever",
                     "batch_derivation": {"derived_batch_would_be": 64,
                                          "derived_batch_source_would_be": "declared-requirement"},
                     "kv_cache_memory_bytes": 21474836480, "kv_source": "measured-clamp"})
        _sweep_knobs("8", "21474836480")
        _cm = _set_cm(str(_swp))
        _mm = _cm.get("provenance_mismatch") or []
        ck("★손레버 batch 8 ↔ explorer 재계산 64 → batch_hand_lever 불일치 기재(yaml↔lockset 은 자명 일치)",
           [m["knob"] for m in _mm] == ["batch_hand_lever"] and _mm[0]["declared"] == 8 and _mm[0]["observed"] == 64
           and "derived_batch_would_be" in _mm[0]["observed_source"]
           and _mm[0]["observed_source_kind"] == "declared-requirement"
           and "batch_hand_lever" in ((_cm.get("provenance") or {}).get("compared") or []))
        _ld = _read_json(camp / "cells" / "cm" / "lockset.json")
        _ld["batch_derivation"]["derived_batch_would_be"] = 8
        _write_json(camp / "cells" / "cm" / "lockset.json", _ld)
        _cm = _set_cm(str(_swp))
        ck("★손레버 음성대조: 재계산값이 손값과 같으면 기재 0 건 · 대조 사실은 남는다",
           _cm.get("provenance_mismatch") == []
           and "batch_hand_lever" in ((_cm.get("provenance") or {}).get("compared") or []))
        (camp / "cells" / "cm" / "lockset.json").unlink()
        _cm = _set_cm(None)
        ck("★writer 는 lockset 부재를 거부하지 않고 사유로 기재한다(진입 게이트는 precheck 하나)",
           (_cm.get("provenance") or {}).get("lockset") is None
           and "부재" in ((_cm.get("provenance") or {}).get("lockset_gap") or ""))
        ck("★precheck producer 배선 앵커: broad_search.sh cell 이 --lockset-precheck 를 부른다",
           _bs.is_file() and "--lockset-precheck" in _bs.read_text(encoding="utf-8"))

        # ── hint 발행 사전승인 writer (2026-09-21 · plan_26092119 O6 · SPEC X3) ─────────────────
        #    양성(승인이 적히고 검증기가 읽는다)과 음성대조(4필드 결손·배정 밖·서브·결함 선언은 거부 · 선언
        #    바이트 불변)를 둘 다 둔다. ACTIVE 는 다른 인스턴스(w1)를 가리키게 두어 명시 id 가 이기는지 본다.
        _ha = CAMPAIGNS / "ha"
        _ha.mkdir(parents=True, exist_ok=True)
        _write_json(_ha / "campaign.yaml", {
            "schema_version": 1, "id": "ha", "plan_ref": "docs/plan/p.md",
            "nodes": [{"node_id": "main", "role": "main"}, {"node_id": "sub", "role": "sub"}],
            "assignments": {"main": [{"cell": "h1"}, {"cell": "h2"}, {"cell": "h3"}],
                            "sub": [{"cell": "h9"}]},
            "hint_targets": []})
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")
        _U1, _U2 = "2026-09-21T10:00:00Z", "2026-09-21T11:00:00Z"
        _sayA = '사용자(발화 전사) — "h1·h2 발행 승인"'
        _sayB = '사용자(발화 전사) — "h2·h3 도 발행"'

        def _hbytes(name: str = "ha") -> bytes:
            return (CAMPAIGNS / name / "campaign.yaml").read_bytes()

        def _happ(*extra, cells="h1", say=_sayA, utc=_U1, camp_id="ha"):
            args = ["--hint-approve", "--campaign-id", camp_id, "--node", "main", "--cells", cells,
                    "--approved-by", say, "--utc", utc, *extra]
            return _cli(*args)

        _full = ["--hint-approve", "--campaign-id", "ha", "--node", "main", "--cells", "h1",
                 "--approved-by", _sayA, "--utc", _U1]
        _b0, _w1b = _hbytes(), _hbytes("w1")
        for _flag in ("--node", "--cells", "--approved-by", "--utc"):
            _i = _full.index(_flag)
            _rc, _o, _e = _cli(*(_full[:_i] + _full[_i + 2:]))
            ck(f"★4필드 강제: {_flag} 가 없으면 거부(rc≠0 · 선언 바이트 불변)",
               _rc != 0 and _hbytes() == _b0 and "FAIL" in _e)
        for _lbl, _extra, _kw in (
                ("빈 --cells(백지 승인 ✗)", (), {"cells": " , "}),
                ("주입 모양이 아닌 --utc", (), {"utc": "2026-09-21 10:00"}),
                ("목록 밖 --source", ("--source", "chat"), {}),
                ("--approved-by 의 <<FILL>>", (), {"say": FILL}),
                ("배정 밖 셀(서브 배정 셀을 메인 승인으로)", (), {"cells": "h9"})):
            _rc, _o, _e = _happ(*_extra, **_kw)
            ck(f"★음성대조 {_lbl} 은 거부(선언 바이트 불변)", _rc != 0 and _hbytes() == _b0)
        ck("★음성대조 선언되지 않은 노드는 거부",
           _boom(lambda: writer_hint_approve(_ha, node="ghost", cells=["h1"], approved_by=_sayA, utc=_U1)))

        _rc, _o, _e = _happ()
        _hd = _read_json(_ha / "campaign.yaml")
        ck("★승인이 선언에 전사된다(기본 출처 = 선언 확인 팝업 · 명시 --campaign-id 가 ACTIVE 를 이긴다)",
           _rc == 0 and "wrote(ha)" in _o and _hd["hint_targets"] == [
               {"node_id": "main", "cells": ["h1"],
                "approval": {"approved_by": _sayA, "approved_utc": _U1, "source": "declaration-popup"}}]
           and _hbytes("w1") == _w1b)
        ck("★writer 가 쓴 승인은 검증기를 통과하고 읽는 눈이 그 전사를 돌려준다",
           not validator().hint_target_reasons(_hd)
           and (validator().hint_approval_for(_hd, "h1", "main") or {}).get("approval", {}).get("approved_by") == _sayA
           and validator().hint_approval_for(_hd, "h2", "main") is None)
        _rc, _o, _e = _happ(cells="h1,h2")
        _hd = _read_json(_ha / "campaign.yaml")
        ck("★같은 승인 사건이면 셀을 합친다(항목 하나 · h1 은 멱등 기재)",
           _rc == 0 and len(_hd["hint_targets"]) == 1 and _hd["hint_targets"][0]["cells"] == ["h1", "h2"]
           and "멱등" in _e)
        _b1 = _hbytes()
        _rc, _o, _e = _happ(cells="h1,h2")
        ck("★멱등: 같은 명령을 다시 적으면 선언 바이트가 그대로다", _rc == 0 and _hbytes() == _b1)
        _rc, _o, _e = _happ("--source", "publish-popup", cells="h2,h3", say=_sayB, utc=_U2)
        _hd = _read_json(_ha / "campaign.yaml")
        ck("★다른 승인 사건은 새 항목으로 쌓이고 이미 승인된 셀(h2)은 옛 승인을 지킨다(귀속 불변)",
           _rc == 0 and len(_hd["hint_targets"]) == 2
           and _hd["hint_targets"][0]["cells"] == ["h1", "h2"]
           and _hd["hint_targets"][0]["approval"]["approved_by"] == _sayA
           and _hd["hint_targets"][1] == {"node_id": "main", "cells": ["h3"], "approval": {
               "approved_by": _sayB, "approved_utc": _U2, "source": "publish-popup"}}
           and "옛 승인을 지킨다" in _e
           and (validator().hint_approval_for(_hd, "h2") or {}).get("approval", {}).get("approved_by") == _sayA)
        _b2 = _hbytes()
        _rc, _o, _e = _happ(cells="h2", say='사용자(발화 전사) — "h2 재승인"', utc=_U2)
        ck("★음성대조: 이미 승인된 셀만 다시 승인하면 아무것도 바꾸지 않는다(rc 0 · 바이트 불변)",
           _rc == 0 and _hbytes() == _b2 and "옛 승인을 지킨다" in _e)

        ACTIVE_POINTER.write_text(BOOTSTRAP + "\n", encoding="utf-8")
        _rc, _o, _e = _cli("--hint-approve", "--node", "main", "--cells", "h1", "--approved-by", _sayA,
                           "--utc", _U1)
        ck("★_bootstrap(ACTIVE 부재·명시 없음)이면 no-op 이다(rc 0 · 이름으로 말한다 · 어떤 선언도 불변)",
           _rc == 0 and "no-op" in _e and _hbytes() == _b2 and _hbytes("w1") == _w1b)
        _rc, _o, _e = _happ(camp_id=BOOTSTRAP)
        ck("★명시 --campaign-id _bootstrap 도 no-op 이다", _rc == 0 and "no-op" in _e and _hbytes() == _b2)
        ACTIVE_POINTER.write_text("w1\n", encoding="utf-8")

        _hs = CAMPAIGNS / "hs"
        _hs.mkdir(parents=True, exist_ok=True)
        _write_json(_hs / "campaign.yaml", {"schema_version": 1, "id": "hs", "self_role": "sub",
                                            "nodes": [{"node_id": "main", "role": "main"}],
                                            "assignments": {"main": [{"cell": "h1"}]}, "hint_targets": []})
        _bs0 = _hbytes("hs")
        ck("★음성대조 서브 인스턴스(self_role=sub)에는 승인을 적지 않는다(발행은 메인 소관)",
           _happ(camp_id="hs")[0] != 0 and _hbytes("hs") == _bs0)
        _hx = CAMPAIGNS / "hx"
        _hx.mkdir(parents=True, exist_ok=True)
        _write_json(_hx / "campaign.yaml", {
            "schema_version": 1, "id": "hx", "nodes": [{"node_id": "main", "role": "main"}],
            "assignments": {"main": [{"cell": "h1"}, {"cell": "h2"}]},
            "hint_targets": [{"arch": "gb10-main-sim-h100", "node_id": "main", "cells": []}]})
        _bx0 = _hbytes("hx")
        _rc, _o, _e = _happ(camp_id="hx", cells="h2")
        ck("★음성대조 옛 모양(arch · 빈 cells) 위에는 승인을 얹지 않는다(결함을 먼저 고친다 · 바이트 불변)",
           _rc != 0 and "이미 결함" in _e and "arch" in _e and _hbytes("hx") == _bx0)

        _slh = emit_slice(_ha, "sub", utc="2026-09-21T12:00:00Z")
        ck("★메인 몫 승인만 있으면 서브 파생 선언의 hint_targets 는 비어 있다(남의 몫은 실제로 잘린다)",
           len(_read_json(_ha / "campaign.yaml")["hint_targets"]) == 2 and _slh["hint_targets"] == [])

        # ── 서브 몫 승인 운반 (2026-10-11 · plan_26101104 Step 3 · single 서브 자체 발행) ───────────────
        #    양성: 메인 선언에 `--hint-approve --node sub` → `--emit-slice sub` 가 그 노드 몫만 승인째 싣고, 서브
        #    인스턴스 검증기·읽는 쪽(hint_approval_for)이 그것을 받아들인다. 음성: 메인 몫·다른 노드 몫은 잘리고,
        #    multi(Ray 워커) 서브는 운반하지 않으며, topology 를 모르면 거부하고, 밀반입된 남의 몫은 서브 검증기가 거부한다.
        _hp = CAMPAIGNS / "hp"
        _hp.mkdir(parents=True, exist_ok=True)
        _hp_doc = {"schema_version": 1, "id": "hp", "plan_ref": "docs/plan/p.md",
                   "nodes": [{"node_id": "main", "role": "main", "topology": "single"},
                             {"node_id": "sub", "role": "sub", "topology": "single"}],
                   "assignments": {"main": [{"cell": "m1"}], "sub": [{"cell": "s1"}, {"cell": "s2"}]},
                   "hint_targets": []}
        _write_json(_hp / "campaign.yaml", _hp_doc)
        _rc_m, _, _ = _cli("--hint-approve", "--campaign-id", "hp", "--node", "main", "--cells", "m1",
                           "--approved-by", _sayA, "--utc", _U1)
        _rc_s, _o_s, _e_s = _cli("--hint-approve", "--campaign-id", "hp", "--node", "sub", "--cells", "s1",
                                 "--approved-by", _sayB, "--utc", _U2)
        _hpd = _read_json(_hp / "campaign.yaml")
        ck("★서브 노드 셀도 메인 선언에 승인이 적힌다(--hint-approve --node sub)",
           _rc_m == 0 and _rc_s == 0 and [t["node_id"] for t in _hpd["hint_targets"]] == ["main", "sub"])
        _sls = emit_slice(_hp, "sub", utc="2026-10-11T00:00:00Z")
        ck("★single 서브 파생 선언은 그 노드 몫 hint_targets 를 승인째 싣는다(메인 몫은 잘린다)",
           _sls["hint_targets"] == [{"node_id": "sub", "cells": ["s1"],
                                     "approval": {"approved_by": _sayB, "approved_utc": _U2,
                                                  "source": validator().HINT_APPROVAL_DEFAULT_SOURCE}}])
        ck("★운반은 깊은 복사다(파생 선언을 고쳐도 메인 선언은 그대로)",
           _sls["hint_targets"][0] is not _hpd["hint_targets"][1])
        _vv = validator()
        ck("★서브 인스턴스 검증기가 자기 몫 승인을 받아들인다(결함 0)", _vv.hint_target_reasons(_sls) == [])
        _ap = _vv.hint_approval_for(_sls, "s1", node="sub")
        ck("★서브의 읽는 쪽(hint_approval_for)이 그 셀의 승인을 돌려준다 · 승인 안 된 셀은 None",
           _ap is not None and _ap["approval"]["approved_by"] == _sayB
           and _vv.hint_approval_for(_sls, "s2", node="sub") is None)
        _smug = dict(_sls, hint_targets=_sls["hint_targets"] + [_hpd["hint_targets"][0]])
        ck("★음성대조 서브 인스턴스에 밀반입된 메인 몫 승인 → 거부 · 어떤 승인도 읽지 않는다(fail-closed)",
           any("다른 노드" in r for r in _vv.hint_target_reasons(_smug))
           and _vv.hint_approval_for(_smug, "s1", node="sub") is None)
        ck("★메인 노드로 자른 파생 선언은 hint_targets 를 싣지 않는다(파생 선언은 서브용 지시서)",
           emit_slice(_hp, "main", utc="t")["hint_targets"] == [])
        _hpm = dict(_hpd, nodes=[{"node_id": "main", "role": "main", "topology": "multi"},
                                 {"node_id": "sub", "role": "sub", "topology": "multi", "rank": 1}])
        _write_json(_hp / "campaign.yaml", _hpm)
        _slm = emit_slice(_hp, "sub", utc="t")
        ck("★음성대조 multi(Ray 워커) 서브는 승인이 있어도 운반하지 않는다(자체 발행 ✗)", _slm["hint_targets"] == [])
        ck("★음성대조 multi 서브 인스턴스에 밀반입된 자기 몫 승인 → 거부",
           any("자체 발행하지 않는다" in r
               for r in _vv.hint_target_reasons(dict(_slm, hint_targets=_sls["hint_targets"]))))
        _write_json(_hp / "campaign.yaml", dict(_hpd, nodes=[{"node_id": "main", "role": "main", "topology": "single"},
                                                             {"node_id": "sub", "role": "sub"}]))
        ck("★음성대조 그 노드의 topology 를 모르는데 그 노드 몫 승인이 있으면 추측하지 않고 거부",
           _boom(lambda: emit_slice(_hp, "sub", utc="t")))
        # 교차검증: 서브에도 가는 검증기의 손사본 ↔ 메인 계약기의 파생(단일 소유 불가 시 차선)
        import importlib.util as _ilu
        _nrc_spec = _ilu.spec_from_file_location("_ci_nrc", Path(__file__).resolve().parent / "node_role_contract.py")
        _nrc = _ilu.module_from_spec(_nrc_spec)
        _nrc_spec.loader.exec_module(_nrc)
        ck("★검증기 SELF_PUBLISH_TOPOLOGIES = 계약기 sub_self_publishes 파생(교차검증)",
           set(_vv.SELF_PUBLISH_TOPOLOGIES)
           == {t for t in _nrc.TOPOLOGIES if _nrc.sub_self_publishes(t)["value"]})

    CAMPAIGNS, ACTIVE_POINTER, REPO_ROOT = saved
    print("[campaign_init] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--active", action="store_true", help="활성 캠페인 id 를 출력")
    ap.add_argument("--derive", help="producer 출력 경로 파생 (root|config|lockset|cell-status|sweep|relay|relay-root|pending-hitl|evidence|phases)")
    ap.add_argument("--cell"); ap.add_argument("--sweep"); ap.add_argument("--context")
    ap.add_argument("--campaign-id", help="파생 대상 캠페인(생략 시 활성)")
    ap.add_argument("--verify-purge-gate", metavar="PREV_ID", help="직전 인스턴스의 purge 선행조건 검사")
    ap.add_argument("--close", metavar="CAMP_ID",
                    help="활성 캠페인 종결 — purge 게이트와 같은 선행조건을 묻고 ACTIVE 만 _bootstrap 으로 "
                         "되돌린다(인스턴스 보존) · --utc 필수 · 기본 dry-run(--apply 로 실행)")
    ap.add_argument("--residue-scan", action="store_true")
    ap.add_argument("--init", metavar="CAMP_ID", help="새 캠페인 개설")
    ap.add_argument("--plan-ref", help="--init 의 승인 plan 경로(필수)")
    # 반복 지정 가능(2026-09-06). 설계 의도는 "활성 인스턴스 1개"지만 현실은 그렇지 않을 수
    # 있다 — 중단된 캠페인·루트 누출 흡수분이 인스턴스를 둘 이상 남긴다. 단일 id 만 받으면
    # 남은 하나를 지울 정식 경로가 없어져 rm -rf 우회를 부른다(D3: 우회 대신 경로를 고친다).
    ap.add_argument("--purge-previous", metavar="PREV_ID", action="append", default=[],
                    help="--init 과 함께: 먼저 지울 직전 인스턴스(반복 지정 가능)")
    ap.add_argument("--from-slice", metavar="PATH|-",
                    help="--init 과 함께: 메인이 보낸 파생 선언으로 campaign.yaml 을 연다"
                         " (`-` = A2A 지시서 본문의 stdin JSON)")
    ap.add_argument("--ground", action="store_true",
                    help="통제변인 파생 질의로 사서를 부르고 grounding/<utc>.json 에 3메시지로 남긴다 "
                         "· --utc 필수 (policy:LIBRARY_GROUNDING_FAIL_CLOSED C1)")
    ap.add_argument("--terms", default=None,
                    help="--ground 의 질의어를 쉼표로 명시(생략 시 통제변인에서 파생)")
    ap.add_argument("--no-warm-start", action="store_true",
                    help="--ground 전 서가 증분 입고를 생략한다(정지한 서가의 '없음' 은 거짓이다)")
    ap.add_argument("--grounding-check", action="store_true",
                    help="그라운딩 기록이 성립하는지 묻는다 — 진입 백스톱(C2)이 부른다. "
                         "부재는 rc 4(fail-closed) · 활성 캠페인이 없으면 rc 0")
    ap.add_argument("--lockset-precheck", action="store_true",
                    help="셀 출처 precheck(--cell 필수) — lockset 존재 ∧ provenance ∈ 목록이면 rc 0, "
                         "부재·무효면 rc 2(fail-closed). 활성 캠페인이 없으면(_bootstrap) 대상이 아님을 "
                         "밝히고 rc 0. 판정 JSON 은 stdout, 사유는 stderr (broad_search.sh cell 이 부른다)")
    ap.add_argument("--escalation-summary", action="store_true",
                    help="캠페인 종결 HITL 이 읽는 escalation 후보 요약(JSON) — 판정하지 않고 "
                         "센다. 술어 ① 이 선 후보가 있으면 hitl_required=true")
    ap.add_argument("--warm-start-library", action="store_true",
                    help="서가 증분 입고만 수행한다(C4 — 발행기·publish 위상 종료부가 부른다)")
    ap.add_argument("--backfill-from-docs", action="store_true",
                    help="결손 셀의 측정·노드를 스윕 기록·sweep map 에서 사후 수리한다 · --utc 필수 "
                         "(hint 발행 직전 · 차단 ✗ · 남은 결손은 이름을 부른다)")
    ap.add_argument("--assigned-cells", metavar="NODE",
                    help="그 노드에 배정된 셀을 쉼표로 출력한다(스윕 --cells 의 파생 원천). "
                         "손으로 적은 목록은 선언과 갈라진다 — 갈라진 목록은 지도를 거짓말하게 만든다")
    ap.add_argument("--emit-slice", metavar="NODE",
                    help="그 노드 몫만 자른 파생 선언을 낸다(메인 오케스트레이션) · --utc 필수")
    ap.add_argument("--out", metavar="PATH", help="--emit-slice 산출 경로(생략 시 stdout)")
    ap.add_argument("--import-sub", metavar="MIRROR",
                    help="회수 미러(sync_staging/sub_docs)를 메인 인스턴스에 편입한다 · --utc 필수 "
                         "(fetch_sub_docs.sh 종료부가 부른다 — 사후 손저작 대체)")
    ap.add_argument("--write-brief", action="store_true",
                    help="docs/logs/<node>/campaign_brief.json 갱신 · --node --utc 필수 "
                         "(선언된 관측면 — 메인은 이 파일 외에 서브를 읽지 않는다)")
    ap.add_argument("--apply", action="store_true", help="실제로 쓰고 지운다(기본은 dry-run)")
    ap.add_argument("--selftest", action="store_true")

    # ── 수명 전이 (2026-10-17 · plan_26101719). 단일 정문 = `--init <B> --suspend-current [--derived-from <A>]`.
    lc = ap.add_argument_group("수명 전이 (보류·재개·포기·선점 — journey 단일 권위 · 사람 확인 = --approved-by)")
    lc.add_argument("--suspend-current", action="store_true",
                    help="--init 과 함께: 활성 캠페인을 보류하고 새 캠페인을 연다(원자 전이 · 처분 없는 덮어쓰기는 거부)")
    lc.add_argument("--derived-from", metavar="CAMP_ID",
                    help="--init 의 파생 부모(기록되면 자식 종결 시 부모 재개 질의가 1회 걸린다 · 계기 a)")
    lc.add_argument("--preempt", action="store_true",
                    help="진행 중 셀이 있어도 강제 전환(측정 중단 · 서빙 내림 · 그 셀 interrupted · 부분값 증거 ✗)")
    lc.add_argument("--release", action="append", default=[], metavar="CONFIG|none",
                    help="보류 시 정식 teardown 으로 내릴 서빙 config(반복) · 띄운 것이 없으면 `none`")
    lc.add_argument("--dispose", action="append", default=[], metavar="CAMP=keep",
                    help="무관 개설(계기 d) 때 답 없는 보류 캠페인의 처분 전사(반복 · keep 만 — 포기는 --abandon)")
    lc.add_argument("--target-topology", choices=("single", "multi"),
                    help="새 캠페인의 토폴로지(생략 = 체크아웃). 체크아웃과 다르면 1단(보류+개설 대기)까지만 한다")
    lc.add_argument("--reason", help="보류·포기 사유")
    lc.add_argument("--suspend", metavar="CAMP_ID",
                    help="단독 보류(서브 A2A 보류 지시의 실행부 · 개설 없음) · --release --approved-by --utc")
    lc.add_argument("--resume-campaign", metavar="CAMP_ID",
                    help="보류 캠페인 재개(핀·digest 대조 · 토폴로지는 체크아웃과 같아야 한다) · --approved-by --utc")
    lc.add_argument("--accept-drift", action="store_true", help="--resume-campaign: 보류 동안 바뀐 핀을 사람이 수용")
    lc.add_argument("--abandon", metavar="CAMP_ID",
                    help="포기 종결(축소 게이트: 측정 증거 도착 + --summary-testlog) · --reason --approved-by --utc")
    lc.add_argument("--summary-testlog", metavar="PATH", help="--abandon 의 포기 요약 testlog(저장소 상대경로)")
    lc.add_argument("--answer-query", metavar="CAMP_ID",
                    help="계기 질의에 '보류 유지' 로 답한다(--query-decision keep) · --approved-by --utc")
    lc.add_argument("--query-decision", choices=("keep",), default=None)
    lc.add_argument("--sub-echo", metavar="CAMP_ID",
                    help="서브가 자기 인스턴스에서 보류/재개했다는 echo 기록 · --node --control --utc")
    lc.add_argument("--control", choices=SUB_CONTROLS)
    lc.add_argument("--sub-unconverged-accept", metavar="CAMP_ID",
                    help="종결된 캠페인의 서브 미도달을 사람이 수용(purge 게이트 탈출구) · --node --approved-by --utc")
    lc.add_argument("--lifecycle", nargs="?", const="*", metavar="CAMP_ID",
                    help="수명 fold 를 JSON 으로 낸다(생략 = 전 인스턴스 · 읽기 전용)")

    # ── 단일 writer (2026-09-07 · plan_26090715 §4.1). ACTIVE=_bootstrap 이면 전부 no-op.
    w = ap.add_argument_group("writer (campaigns/<id>/ 에 바이트를 쓰는 유일한 자리)")
    w.add_argument("--phase-set", metavar="PHASE",
                   help="phase 상태 기록 (build|serve|bench|publish) · --node 필수")
    w.add_argument("--node", help="--phase-set 의 노드 id(= manifest role 슬러그) · --hint-approve 의 선언 노드")
    w.add_argument("--state", help="--phase-set 의 상태 (pending|running|done|failed)")
    w.add_argument("--proof-predicate", help="무엇을 물었는가")
    w.add_argument("--proof-ok", action="store_true", help="관측 결과 참")
    w.add_argument("--proof-source", help="그 판정을 낸 명령·파일 (proof-ok 에는 필수)")
    w.add_argument("--started-utc"); w.add_argument("--ended-utc")
    w.add_argument("--cell-set", metavar="CELL", help="셀 상태 기록 · --outcome 필수")
    w.add_argument("--cell-materialize", metavar="CELL",
                   help="배정 셀의 셀 입력(config.yaml·lockset.json·cell.status.json)을 뼈대에서 만든다 · 파생값은 서빙 yaml·"
                        "env·manifest·/proc/meminfo · 선언 축은 --set · --utc 필수 · 덮어쓰기 ✗(plan_26101622 F1)")
    w.add_argument("--set", dest="mat_set", action="append", default=[], metavar="K=V",
                   help="--cell-materialize 의 선언 축(반복 · 닫힌 목록 MATERIALIZE_SET_KEYS · 요구 없음 = K=null)")
    w.add_argument("--outcome", help="pending|measured|serve_failed|build_failed|void")
    w.add_argument("--version"); w.add_argument("--model")
    w.add_argument("--measurement-decode-tps"); w.add_argument("--measurement-source")
    w.add_argument("--sweep-state", metavar="PATH",
                   help="--cell-set 이 이 sweep 기록에서 측정값·이미지 좌표를 직접 읽는다 "
                        "(호출부가 값을 나르지 않는다 · 없으면 결손 기재)")
    w.add_argument("--escalation-add", action="store_true",
                   help="escalation 후보 한 줄을 원장에 쌓는다 · --cell --utc 필수 "
                        "(--symptom 반복 · 술어 ① 은 escalation_predicate.py 가 판정한다)")
    w.add_argument("--symptom", action="append", default=[],
                   help="--escalation-add 의 관측 증상(반복 가능). 호스트 평면 사인은 술어 ① 을 "
                        "만들지 못하지만 **기록은 남는다**(부재와 무관은 다른 사실)")
    w.add_argument("--events", action="append", default=[],
                   help="--escalation-add 의 시각 대조용 노드 블랙박스 events jsonl(반복 가능)")
    w.add_argument("--window-start-utc", help="--escalation-add 의 대조 구간 시작")
    w.add_argument("--window-end-utc", help="--escalation-add 의 대조 구간 끝")
    w.add_argument("--image-digest", metavar="SHA",
                   help="이 셀을 돌린 이미지의 digest(태그 ✗ — 태그는 이름이고 digest 는 내용이다). "
                        "--sweep-state 로 파생되면 그쪽이 이기고, 둘이 다르면 거부한다")
    w.add_argument("--image-tag", metavar="TAG", help="사람이 읽는 이름(비교 권위 아님)")
    w.add_argument("--authored-by", metavar="NODE",
                   help="--phase-set: 이 진행표를 적은 주체(서브 자기저작 vs 메인 사후 재저작 판별)")
    w.add_argument("--void-reason"); w.add_argument("--void-reason-source")
    w.add_argument("--lite-raw", metavar="PATH",
                   help="--cell-set: 이번 셀의 lite raw — lite 판정을 셀 상태에 싣고 ② 면 cap 차감 1(멱등)·재발동 제안을 적는다")
    w.add_argument("--reentry-decide", metavar="CELL",
                   help="재발동 제안에 대한 사람 결정 기록 · --decision --approved-by --utc 필수(approve 는 --route 도)")
    w.add_argument("--decision", choices=("approve", "decline"))
    w.add_argument("--route", choices=tuple(REENTRY_ROUTES))
    w.add_argument("--axis-citation", help="이 셀이 움직인 축의 근거(layer-2 자율)")
    w.add_argument("--next-intent", help="여정 한 줄 — 다음에 무엇을 할 참인가")
    w.add_argument("--evidence-add", action="store_true", help="증거 포인터 추가 · --kind --path 필수")
    w.add_argument("--kind", help=f"증거 종류 {EVIDENCE_KINDS}")
    w.add_argument("--path", help="저장소 상대경로(실재해야 한다)")
    w.add_argument("--unfreeze", action="store_true", help="동결된 스냅샷에 사람이 명시로 추가")
    w.add_argument("--evidence-untag", action="store_true",
                   help="포인터의 셀 귀속 해제(포인터는 남김 · untagged[] 이력) · --kind --path --cell --untag-reason --utc 필수")
    w.add_argument("--untag-reason", help="--evidence-untag 의 사유(필수)")
    w.add_argument("--evidence-relocate", action="store_true",
                   help="포인터 경로 이전(문서 개명 · 옛 경로 부재 ∧ 새 경로 실재일 때만 · relocated[] 이력) · "
                        "--kind --path(옛) --to(새) --relocate-reason --utc 필수")
    w.add_argument("--to", dest="relocate_to", help="--evidence-relocate 의 새 저장소 상대경로")
    w.add_argument("--relocate-reason", help="--evidence-relocate 의 사유(필수)")
    w.add_argument("--correct-reference", action="store_true",
                   help="--evidence-relocate: sub relay_summary의 잘못된 캠페인 relay 포인터를 docs/testlog 요약으로 정정. "
                        "원본 보존·node/cell 불변·이력 기록 · --node --cell 필수")
    w.add_argument("--evidence-prune-stubs", action="store_true",
                   help="뼈대에서 딸려온 <<FILL>> 스텁 포인터를 정식 경로로 제거한다 "
                        "(값이 든 포인터는 건드리지 않는다 · 손삭제 대체)")
    w.add_argument("--freeze-evidence", action="store_true",
                   help="캠페인 전체 증거 스냅샷을 동결한다(사람 명시 연산 · hint 발행기는 부르지 않는다 — "
                        "셀 단위 발행 2026-09-21 D9) · --utc 필수")
    w.add_argument("--assign-set", metavar="NODE",
                   help="assignments[NODE] 개정(pending 셀만 · 4필드 필수) · --assign-cells c1[:MODE],c2,… "
                        "--assign-reason --assign-evidence --assign-approved-by --utc")
    w.add_argument("--assign-cells", default=None)
    w.add_argument("--assign-reason"); w.add_argument("--assign-evidence"); w.add_argument("--assign-approved-by")
    w.add_argument("--slice-update", metavar="PATH|-",
                   help="서브: 메인 파생 선언으로 자기 인스턴스 배정을 바꾼다(relay reslice 의 실행부) · --utc --apply")
    w.add_argument("--revise", action="store_true",
                   help="layer-1 control_variables append-only 개정(메인 단일 창구)")
    w.add_argument("--revise-set", action="append", default=[], metavar="K=V",
                   help="개정할 키=값(반복)")
    w.add_argument("--revise-reason"); w.add_argument("--revise-evidence")
    w.add_argument("--revise-approved-by")
    # hint 발행 사전승인(2026-09-21 · plan_26092119 O6). --node --cells --approved-by --utc 넷 필수.
    w.add_argument("--hint-approve", action="store_true",
                   help="hint 발행 사전승인을 선언 hint_targets 에 전사한다 · --node --cells --approved-by --utc "
                        "필수(넷 중 하나라도 없으면 승인이 아니다) · 메인 선언 전용")
    w.add_argument("--cells", metavar="A,B",
                   help="--hint-approve 의 승인 셀(쉼표 · 명시 열거 · 그 노드의 배정 셀이어야 한다)")
    w.add_argument("--approved-by", metavar="TEXT",
                   help="--hint-approve · --reentry-decide 의 사람 발화 전사(이름·연락처 대신 역할 — hint 는 봉인 시 페이로드로 복사된다)")
    w.add_argument("--source", metavar="declaration-popup|publish-popup",
                   help="--hint-approve 의 승인 자리(생략 = declaration-popup · 선언 확인 팝업이 O6 기본 경로)")
    w.add_argument("--utc", help="시각은 주입만 받는다(벽시계 금지)")
    w.add_argument("--resume-brief", action="store_true",
                   help="인스턴스만 읽고 재개 요약을 낸다(README 읽기 순서 0번)")
    a = ap.parse_args(argv)
    try:
        if a.correct_reference and not a.evidence_relocate:
            raise WriterRefusal("--correct-reference 는 --evidence-relocate와 함께만 쓴다")
        if a.selftest:
            return _selftest()
        if a.active:
            print(active_campaign_id()); return 0
        if a.derive:
            print(derive_path(a.derive, cell=a.cell, sweep=a.sweep, context=a.context,
                              camp_id=a.campaign_id)); return 0
        if a.resume_brief:
            sys.stdout.write(resume_brief(a.campaign_id)); return 0
        if a.warm_start_library:
            print(f"[campaign_init] {warm_start_library()}"); return 0
        if a.grounding_check:
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                print("[campaign_init] 그라운딩 검사 생략 — 활성 캠페인 없음(_bootstrap)",
                      file=sys.stderr)
                return 0
            reasons = grounding_reasons(tgt[0])
            if reasons:
                print("[campaign_init] 그라운딩 백스톱: 통과하지 못했다 "
                      "(policy:LIBRARY_GROUNDING_FAIL_CLOSED C2)", file=sys.stderr)
                for r in reasons:
                    print(f"  - {r}", file=sys.stderr)
                return 4
            print("[campaign_init] 그라운딩 백스톱 통과"); return 0
        if a.lockset_precheck:
            if not a.cell:
                raise WriterRefusal("--lockset-precheck 는 --cell 이 필요하다(셀 없는 출처는 없다)")
            verdict = lockset_precheck(a.cell, a.campaign_id)
            print(json.dumps(verdict, ensure_ascii=False))
            if verdict["status"] == "refused":
                print(f"[campaign_init] 셀 출처 precheck 거부 — {verdict['reason']}", file=sys.stderr)
                return 2
            if verdict["status"] == "not_applicable":
                print(f"[campaign_init] ⓘ 셀 출처 precheck 대상 아님 — {verdict['reason']}",
                      file=sys.stderr)
            return 0
        if a.assigned_cells:
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                raise PurgeGateRefusal("활성 캠페인이 없다(_bootstrap) — 배정을 물을 선언이 없다")
            cells = validator().assigned_cells(read_declaration(tgt[0]), a.assigned_cells)
            if not cells:
                raise PurgeGateRefusal(
                    f"assignments[{a.assigned_cells!r}] 가 비었다 — 이 노드에 배정된 셀이 없다")
            print(",".join(cells)); return 0
        if a.emit_slice:
            if not a.utc:
                raise WriterRefusal("--emit-slice 는 --utc 가 필요하다(시각은 주입만 받는다)")
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                raise WriterRefusal("활성 캠페인이 없다(_bootstrap) — 자를 선언이 없다")
            doc = emit_slice(tgt[0], a.emit_slice, utc=a.utc)
            blob = json.dumps(doc, ensure_ascii=False, indent=2) + "\n"
            if a.out:
                Path(a.out).parent.mkdir(parents=True, exist_ok=True)
                Path(a.out).write_text(blob, encoding="utf-8")
                print(f"[campaign_init] slice({a.emit_slice}) → {a.out}")
            else:
                sys.stdout.write(blob)
            return 0
        if a.escalation_summary:
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                print(json.dumps({"campaign_id": None, "total": 0, "hitl_required": False,
                                  "_note": "ACTIVE=_bootstrap — 캠페인 밖에는 후보가 없다"},
                                 ensure_ascii=False))
                return 0
            print(json.dumps(escalation_summary(tgt[0]), ensure_ascii=False, indent=2))
            return 0
        if a.write_brief:
            if not a.node or not a.utc:
                raise WriterRefusal("--write-brief 는 --node 와 --utc 가 필요하다")
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                print("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에는 진행이 없다)",
                      file=sys.stderr)
                return 0
            print(f"[campaign_init] brief → "
                  f"{_rel(write_campaign_brief(tgt[0], node=a.node, utc=a.utc))}")
            return 0
        if a.lifecycle:
            if a.lifecycle == "*":
                print(json.dumps({"line": lifecycle_line(), "campaigns": lifecycle_table()},
                                 ensure_ascii=False, indent=2))
            else:
                print(json.dumps(fold_lifecycle(campaigns_dir() / a.lifecycle), ensure_ascii=False, indent=2))
            return 0
        mode = "APPLIED" if a.apply else "DRY-RUN"
        if a.suspend:
            _require(a.utc, a.approved_by, "--suspend")
            event, wrote = suspend_campaign(a.suspend, utc=a.utc, approved_by=a.approved_by,
                                            reason=a.reason or "단독 보류", releases=a.release,
                                            preempt=a.preempt, apply=a.apply)
            print(f"[campaign_init] {mode} suspended {a.suspend} · 내린 자원 {event['released']} · "
                  f"중단 셀 {event['interrupted_cells']} · {' '.join(wrote)}")
            return 0
        if a.resume_campaign:
            for m in resume_campaign(a.resume_campaign, utc=a.utc, approved_by=a.approved_by,
                                     accept_drift=a.accept_drift, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        if a.abandon:
            for m in abandon_campaign(a.abandon, utc=a.utc, approved_by=a.approved_by, reason=a.reason,
                                      summary_testlog=a.summary_testlog, releases=a.release, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        if a.answer_query:
            for m in answer_query(a.answer_query, decision=a.query_decision or "", utc=a.utc,
                                  approved_by=a.approved_by, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        if a.sub_echo:
            if not a.node or not a.control:
                raise TransitionRefusal("--sub-echo 는 --node 와 --control 이 필요하다")
            for m in record_sub_echo(a.sub_echo, node=a.node, control=a.control, utc=a.utc, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        if a.sub_unconverged_accept:
            if not a.node:
                raise TransitionRefusal("--sub-unconverged-accept 는 --node 가 필요하다")
            for m in accept_sub_unconverged(a.sub_unconverged_accept, node=a.node, utc=a.utc,
                                            approved_by=a.approved_by, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        if a.slice_update:
            for m in slice_update(a.slice_update, utc=a.utc, apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        writer_ops = (a.phase_set, a.cell_set, a.evidence_add, a.freeze_evidence, a.revise, a.assign_set,
                      a.evidence_prune_stubs, a.import_sub, a.backfill_from_docs, a.ground,
                      a.escalation_add, a.hint_approve, a.evidence_untag, a.evidence_relocate,
                      a.reentry_decide, a.cell_materialize)
        if any(writer_ops):
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                print("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)",
                      file=sys.stderr)
                return 0
            base, camp = tgt
            # 수명 가드(D13·D15) — 보류·미결·잔재에는 진행성 쓰기를, unknown 에는 모든 쓰기를 거부한다.
            guard_writer_ops(base, {"phase_set": a.phase_set, "cell_set": a.cell_set,
                                    "cell_materialize": a.cell_materialize,
                                    "reentry_decide": a.reentry_decide, "assign_set": a.assign_set})
            wrote: list[str] = []
            if a.phase_set:
                if not a.node and a.cell:
                    # 실행기 종료부(lite_bench·sweep_bench `--campaign-cell`)는 노드를 나르지 않는다 — 배정에서 파생한다
                    # (plan_26101722 K5 · 두 번째 파생을 호출부에 만들지 않는다). 배정 밖 셀은 아래에서 거부된다.
                    a.node = node_of_cell(base, a.cell)
                    if not a.node:
                        raise WriterRefusal(f"셀 {a.cell!r} 이 assignments 에 없다 — 노드를 파생하지 못했다(이름이 곧 증거 연결)")
                if not a.node or not a.state:
                    raise WriterRefusal("--phase-set 은 --node(또는 배정된 --cell)와 --state 가 필요하다")
                if a.phase_set == "publish" and a.state == "done":
                    # C4 — 발행이 끝난 그 자리에서 서가에 넣는다. 발행과 입고가 갈라지면 서가는
                    # 늘 한 캠페인만큼 늦고, 그 늦은 서가의 "없음" 은 거짓이다.
                    print(f"[campaign_init] 서가 입고(C4) — {warm_start_library()}", file=sys.stderr)
                wrote.append(_rel(writer_set_phase(
                    base, node=a.node, phase=a.phase_set, state=a.state,
                    predicate=a.proof_predicate, ok=a.proof_ok, source=a.proof_source,
                    cell_id=a.cell, started_utc=a.started_utc, ended_utc=a.ended_utc,
                    authored_by=a.authored_by)))
            if a.cell_materialize:
                _sets = {}
                for kv in a.mat_set:
                    if "=" not in kv:
                        raise WriterRefusal(f"--set 은 K=V 다: {kv!r}")
                    k, v = kv.split("=", 1)
                    _sets[k.strip()] = v
                wrote += [_rel(p) for p in writer_materialize_cell(
                    base, cell=a.cell_materialize, repo=REPO_ROOT, sets=_sets, utc=a.utc)]
            if a.cell_set:
                if not a.outcome:
                    raise WriterRefusal("--cell-set 은 --outcome 이 필요하다")
                cpath, jpath = writer_set_cell(
                    base, cell=a.cell_set, outcome=a.outcome, node=a.node,
                    version=a.version, model=a.model,
                    decode_tps=a.measurement_decode_tps,
                    measurement_source=a.measurement_source,
                    void_reason=a.void_reason, void_reason_source=a.void_reason_source,
                    axis_citation=a.axis_citation, next_intent=a.next_intent, utc=a.utc,
                    sweep_state=a.sweep_state,
                    image_digest=a.image_digest, image_tag=a.image_tag,
                    lite_raw=a.lite_raw)
                wrote.append(_rel(cpath))
                if jpath is not None:
                    wrote.append(_rel(jpath))
            if a.reentry_decide:
                rpath, rjpath = writer_decide_reentry(
                    base, cell=a.reentry_decide, decision=a.decision, route=a.route,
                    approved_by=a.approved_by, utc=a.utc)
                wrote += [_rel(rpath), _rel(rjpath)]
            if a.escalation_add:
                if not a.cell or not a.utc:
                    raise WriterRefusal("--escalation-add 는 --cell 과 --utc 가 필요하다")
                verdict = judge_escalation_predicate(
                    a.symptom, events=a.events,
                    started_utc=a.window_start_utc, ended_utc=a.window_end_utc)
                epath = writer_append_escalation(base, {
                    "utc": a.utc, "cell_id": a.cell, "node_id": a.node,
                    "symptoms": list(a.symptom),
                    "predicate_1": verdict["predicate_1"],
                    "model_plane_standing": verdict["model_plane_standing"],
                    "model_plane_refuted": verdict["model_plane_refuted"],
                    "host_plane_ignored": verdict["host_plane_ignored"],
                    "unknown_symptoms": verdict["unknown_symptoms"],
                    "evidence_source": verdict["evidence_source"],
                    "reasons": verdict["reasons"],
                    "source": "campaign_init --escalation-add"})
                wrote.append(_rel(epath))
            if a.evidence_add:
                if not a.kind or not a.path:
                    raise WriterRefusal("--evidence-add 는 --kind 와 --path 가 필요하다")
                wrote.append(_rel(writer_add_evidence(
                    base, kind=a.kind, path_rel=a.path, cell_id=a.cell, node=a.node,
                    unfreeze=a.unfreeze)))
            if a.import_sub:
                if not a.utc:
                    raise WriterRefusal("--import-sub 는 --utc 가 필요하다(시각은 주입만 받는다)")
                _w, _warn = import_sub_mirror(base, Path(a.import_sub), utc=a.utc,
                                              unfreeze=a.unfreeze)
                wrote += _w
                for _x in _warn:
                    print(f"[campaign_init] ⚠ {_x}", file=sys.stderr)
            if a.ground:
                if not a.utc:
                    raise WriterRefusal("--ground 는 --utc 가 필요하다(시각은 주입만 받는다)")
                # ★ 2026-09-12 (plan_26091210 A13): 여기 있던 `else "single"` 침묵 폴백을 걷어냈다.
                #   선언이 비거나 오염됐을 때 조용히 single 을 적었고, **그 거짓이 실제로 기록됐다** —
                #   활성 캠페인의 그라운딩 기록이 `request.topology: single` 을 들고 있는데 선언도
                #   브랜치도 multi 였다. 그라운딩은 서빙 진입의 선행조건이므로(정직한 공백은 통과·
                #   거절만 차단) 그 자리에 **거짓 사실**이 실리면 fail-closed 가 무의미해진다.
                #   폴백이 아니라 거부가 옳다: 선언을 고치는 것은 사람의 일이다.
                _topo = (read_declaration(base).get("control_variables") or {}).get("topology")
                if _topo not in ("single", "multi"):
                    raise WriterRefusal(
                        "--ground 는 활성 캠페인의 control_variables.topology 를 요구한다 — "
                        f"현재 값 {_topo!r}. 기본값으로 대체하지 않는다(그라운딩 기록에 거짓 사실이 "
                        "실리면 fail-closed 백스톱이 무의미해진다). 선언을 먼저 고쳐라.")
                _gp, _gd = ground_campaign(
                    base, utc=a.utc, topology=_topo,
                    terms=[t.strip() for t in (a.terms or "").split(",") if t.strip()] or None,
                    warm_start=not a.no_warm_start)
                wrote.append(_rel(_gp))
                print(f"[campaign_init] 그라운딩 · {_gd['library_intake']} · "
                      f"판정={(_gd['attestation'] or {}).get('status')} "
                      f"참조={len((_gd.get('export') or {}).get('references') or [])}건",
                      file=sys.stderr)
            if a.backfill_from_docs:
                # 시각 인자를 요구하지 않는다 — 이 연산은 어떤 타임스탬프도 쓰지 않는다. 안 쓰는
                # 값을 요구하면 호출부가 아무 문자열이나 넣게 되고, 그 순간 그 필드는 거짓이 된다.
                _rep, _gaps = backfill_from_docs(base, utc=a.utc)
                wrote += _rep
                for _g in _gaps:
                    print(f"[campaign_init] ⚠ 남은 결손 — {_g}", file=sys.stderr)
            if a.evidence_untag:
                if not (a.kind and a.path and a.cell):
                    raise WriterRefusal("--evidence-untag 는 --kind --path --cell 이 필요하다")
                wrote.append(_rel(writer_untag_evidence(base, kind=a.kind, path_rel=a.path, cell_id=a.cell,
                                                        reason=a.untag_reason, utc=a.utc, unfreeze=a.unfreeze)))
            if a.evidence_relocate:
                if not (a.kind and a.path and a.relocate_to):
                    raise WriterRefusal("--evidence-relocate 는 --kind --path --to 가 필요하다")
                wrote.append(_rel(writer_relocate_evidence(base, kind=a.kind, path_from=a.path, path_to=a.relocate_to,
                                                           reason=a.relocate_reason, utc=a.utc, unfreeze=a.unfreeze,
                                                           correct_reference=a.correct_reference,
                                                           node=a.node, cell_id=a.cell)))
            if a.evidence_prune_stubs:
                epath, dropped = writer_prune_stubs(base, unfreeze=a.unfreeze)
                wrote.append(_rel(epath))
                print(f"[campaign_init] 스텁 포인터 {len(dropped)}건 제거:", file=sys.stderr)
                for d in dropped:
                    print(f"  - {d}", file=sys.stderr)
            if a.freeze_evidence:
                if not a.utc:
                    raise WriterRefusal("--freeze-evidence 는 --utc 가 필요하다(벽시계 금지)")
                wrote.append(_rel(writer_freeze_evidence(base, a.utc)))
            if a.assign_set:
                if a.assign_cells is None:
                    raise WriterRefusal("--assign-set 은 --assign-cells 가 필요하다(빈 목록은 \"\" 로 명시)")
                _ap, _rev = writer_assign_set(base, node=a.assign_set, items=_parse_assign_cells(a.assign_cells),
                                              reason=a.assign_reason, evidence=a.assign_evidence,
                                              approved_by=a.assign_approved_by, utc=a.utc)
                wrote.append(_rel(_ap))
                if _rev["node"] in _sub_nodes(base):
                    print(f"[campaign_init] 서브 재배정은 A2A 제어명령으로 전한다: relay.py --campaign-control reslice "
                          f"--campaign-id {camp} --node {_rev['node']} … (echo 가 오기 전까지 '재배정 미수렴')",
                          file=sys.stderr)
            if a.revise:
                vals = {}
                for kv in a.revise_set:
                    k, _, v = kv.partition("=")
                    if not k or not _:
                        raise WriterRefusal(f"--revise-set 은 K=V 형식이다: {kv!r}")
                    vals[k.strip()] = v.strip()
                if not vals:
                    raise WriterRefusal("--revise 는 --revise-set K=V 가 하나 이상 필요하다")
                wrote.append(_rel(writer_add_revision(
                    base, values=vals, reason=a.revise_reason, evidence=a.revise_evidence,
                    approved_by=a.revise_approved_by, utc=a.utc)))
            if a.hint_approve:
                _hp, _hr = writer_hint_approve(
                    base, node=a.node, cells=(a.cells or "").split(","), approved_by=a.approved_by,
                    utc=a.utc, source=a.source)
                if _hr["changed"]:
                    wrote.append(_rel(_hp))
                    print(f"[campaign_init] hint 사전승인 hint_targets[{_hr['index']}] ← {_hr['added']}",
                          file=sys.stderr)
                if _hr["already"]:
                    print(f"[campaign_init] 같은 승인으로 이미 적힌 셀(멱등): {_hr['already']}", file=sys.stderr)
                for _k in _hr["kept"]:
                    _ap = _k.get("approval") or {}
                    print(f"[campaign_init] ⓘ {_k['cell']!r} 은 이미 hint_targets[{_k['index']}] 가 승인했다"
                          f"({_ap.get('source')} · {_ap.get('approved_utc')}) — 옛 승인을 지킨다"
                          f"(귀속을 조용히 바꾸지 않는다)", file=sys.stderr)
            print(f"[campaign_init] wrote({camp}): " + " ".join(wrote))
            return 0
        if a.residue_scan:
            out = residue_scan()
            print(json.dumps(out, ensure_ascii=False, indent=2))
            if a.utc:
                wrote = residue_write(out, a.utc, a.campaign_id)
                print(f"[campaign_init] residue → {_rel(wrote) if wrote else '(활성 캠페인 없음 — 미기재)'}",
                      file=sys.stderr)
            else:
                print("[campaign_init] --utc 가 없어 residue.json 에 남기지 않았다"
                      "(시각은 주입만 받는다 — 벽시계 금지)", file=sys.stderr)
            return 0
        if a.verify_purge_gate:
            reasons = purge_gate_reasons(a.verify_purge_gate)
            if reasons:
                print("[campaign_init] purge 게이트 닫힘", file=sys.stderr)
                for r in reasons:
                    print(f"  - {r}", file=sys.stderr)
                return 1
            print("[campaign_init] purge 게이트 열림"); return 0
        if a.close:
            msg = close_campaign(a.close, utc=a.utc, apply=a.apply)
            mode = "APPLIED" if a.apply else "DRY-RUN"
            print(f"[campaign_init] {mode} {msg}"); return 0
        if a.init:
            if not a.plan_ref:
                print("[campaign_init] FAIL --init 은 --plan-ref 가 필요하다(승인 없는 개설 ✗)", file=sys.stderr)
                return 2
            # 단일 정문(2026-10-17 · plan_26101719 D2). purge(인스턴스마다 따로 묻는다)·_bootstrap 대기실 정리·scaffold 는
            # 정문 안에서 같은 순서로 돈다 — 검사 전부가 끝난 뒤에만 쓴다.
            _disp = {}
            for kv in a.dispose:
                k, _, v = kv.partition("=")
                if not k or not _:
                    raise TransitionRefusal(f"--dispose 는 CAMP=keep 형식이다: {kv!r}")
                _disp[k.strip()] = v.strip()
            for m in open_campaign(a.init, a.plan_ref, utc=a.utc, approved_by=a.approved_by,
                                   suspend_current=a.suspend_current, derived_from=a.derived_from,
                                   preempt=a.preempt, releases=a.release, dispositions=_disp,
                                   target_topology=a.target_topology, reason=a.reason,
                                   purge_previous=a.purge_previous, from_slice=a.from_slice,
                                   apply=a.apply):
                print(f"[campaign_init] {mode} {m}")
            return 0
        ap.print_help(); return 2
    except (PurgeGateRefusal, WriterRefusal) as exc:
        print(f"[campaign_init] FAIL {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
