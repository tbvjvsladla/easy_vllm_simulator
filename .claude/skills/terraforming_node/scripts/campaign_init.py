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
4. **단일 writer** (`--phase-set`·`--cell-set`·`--evidence-add`·`--revise`)와 **읽는 눈**
   (`--resume-brief`). 2026-09-07 신설 · plan_26090715 §4.1·§4.4.
   왜: 거처(campaigns/)와 게이트(purge)는 계약대로 섰는데 **채우는 손이 대화 기억**이었다 —
   README 가 재개 에이전트에게 읽으라는 네 아티팩트의 저장소 내 producer 가 0 이었고,
   라이브에선 세션과 함께 소멸하는 스크래치패드 스크립트가 썼다(audit_26090708 §1.1).
   포맷 소유는 여기 하나이고 호출부는 각 phase 의 실제 실행 스크립트다(포맷 소유 1 · 호출부 N).

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
            # 메인이 보낸 파생 선언으로 연다. 서브는 이것 하나로 착수하며 메인의 산출물을
            # 기다리지 않는다 — 기다리면 그것은 자율이 아니라 종속이다(사용자 정정 2026-09-08).
            slice_doc = _read_json(Path(from_slice))
            if not isinstance(slice_doc, dict):
                raise PurgeGateRefusal(f"파생 선언을 읽지 못했다: {from_slice}")
            if slice_doc.get("id") != camp_id:
                raise PurgeGateRefusal(
                    f"파생 선언의 id({slice_doc.get('id')!r})와 개설 이름({camp_id!r})이 다르다 "
                    f"— 이름이 곧 증거 연결이다")
            doc = {k: v for k, v in slice_doc.items() if not k.startswith("_")}
        doc["id"] = camp_id
        doc["plan_ref"] = doc.get("plan_ref") if from_slice else plan_ref
        (dest / "campaign.yaml").write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8")
        # 증거 스냅샷도 같은 처방 + 자기 이름을 채운다. `campaign_id: <<FILL>>` 이 남아 있었고
        # writer 의 setdefault 는 그것을 덮지 않았다 — 부재가 아니라 **빈칸**이라 조용히 살아남았다.
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
CELL_OUTCOMES = ("pending", "measured", "serve_failed", "build_failed", "void")
EVIDENCE_KINDS = ("certificate", "bench_report", "sweep_map", "testlog", "devlog", "simlog",
                  "relay_summary")
FILL = "<<FILL>>"
JOURNEY_NAME = "journey.jsonl"


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


def writer_set_cell(base: Path, *, cell: str, outcome: str, node: str | None,
                    version: str | None, model: str | None,
                    decode_tps: str | None, measurement_source: str | None,
                    void_reason: str | None, void_reason_source: str | None,
                    axis_citation: str | None, next_intent: str | None,
                    utc: str | None, sweep_state: str | None = None
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
    if axis_citation is not None:
        doc["axis_citation"] = axis_citation
    if next_intent is not None:
        doc["next_intent"] = next_intent
    _write_json(path, doc)

    journey = None
    if next_intent:
        # 여정 한 줄은 **같은 트랜잭션**에 실린다 — 새 절차를 만들지 않고 이미 도는 자동쓰기에
        # 인자 하나를 얹는 방식이다(인터뷰 Q4).
        journey = writer_append_journey(base, {
            "utc": utc, "node_id": node, "cell_id": cell, "outcome": outcome,
            "axis_citation": axis_citation, "next_intent": next_intent,
            "source": "campaign_init --cell-set"})
    return path, journey


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


def writer_add_evidence(base: Path, *, kind: str, path_rel: str, cell_id: str | None,
                        node: str | None, unfreeze: bool) -> Path:
    if kind not in EVIDENCE_KINDS:
        raise WriterRefusal(f"kind 는 {EVIDENCE_KINDS} 중 하나여야 한다: {kind!r}")
    _no_fill(path_rel=path_rel)
    if not (REPO_ROOT / path_rel).exists():
        # 증거는 docs 평면에서 **태어난다** — 포인터가 가리키는 것이 없으면 그것은 포인터가 아니다.
        raise WriterRefusal(f"증거가 실재하지 않는다: {path_rel} "
                            f"(증거는 docs 평면에서 태어나고 여기엔 포인터만 둔다)")
    if not node and cell_id:
        node = node_of_cell(base, cell_id)
    declared = [n for n in (read_declaration(base).get("nodes") or []) if isinstance(n, dict)]
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
        raise WriterRefusal(
            f"이 스냅샷은 {doc['frozen_utc']} 에 동결됐다 — publish 위상의 proof.ok 시점 이후에는 "
            f"입력이 바뀌지 않는다(hint 발행의 입력 통로가 흐르면 태그는 불변인데 근거가 움직인다).\n"
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
    """publish 위상의 proof.ok 시점에 스냅샷을 동결한다(인터뷰 Q6)."""
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


# ─────────────────────────────────────────────────────────────────────────────
# 읽는 눈 — resume-brief (2026-09-07 · plan_26090715 §4.4)

def _short(text, limit: int = 160) -> str:
    """브리핑 표시용 절삭. 파일 내용은 온전하다 — **한 화면**이 아니면 읽는 눈이 되지 못한다."""
    t = " ".join(str(text or "").split())
    return t if len(t) <= limit else t[:limit - 1] + "…"


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
    A(f"# resume-brief · campaigns/{camp}")
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
    pending, refuted = [], []
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
        tag = f"[{node_of} · {mode}]" if node_of else "[미배정]"
        A(f"- `{cell}` {tag} — {outcome}" + (f"  ({' · '.join(extra)})" if extra else ""))
        # 상태 파일이 아예 없는 셀도 **남은 작업**이다 — "돌지 않았다"와 "종결했다"를 같은 값으로
        # 접으면 전 셀 미개설인 새 캠페인이 '완주' 로 보인다(부재 ≠ 통과).
        if outcome in ("pending", "(상태파일 없음)", "(미개설)"):
            pending.append(cell)
        if outcome in ("serve_failed", "build_failed", "void"):
            refuted.append((cell, cite, st.get("void_reason")))
    if not listed:
        A("- (배정된 셀 없음)")

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


def emit_slice(base: Path, node: str, *, utc: str) -> dict:
    """`assignments[node]` 만 남긴 파생 선언. 파생 결정은 **메인 오케스트레이션**이다(D12).

    공유하는 것: id·plan_ref·matrix·budgets·control_variables·revisions(echo)·topology_sections.
    자르는 것: nodes(그 노드 하나) · assignments(그 노드 몫) · hint_targets(발행은 메인 소관).
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
        "hint_targets": [],
        "topology_sections": decl.get("topology_sections") or {},
        "self_role": nodes[0].get("role"),
        "authored_by": SLICE_AUTHOR,
        "derived_from": mains[0] if mains else None,
        "derived_utc": utc,
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
    on_disk = sorted(p.name for p in cells_dir.iterdir()
                     if p.is_dir() and p.name != "_cell") if cells_dir.is_dir() else []
    listed = assigned + [c for c in on_disk if c not in assigned]
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
                          if c["cell_outcome"] in (None, "pending")],
        "phases": phases,
        "journey_tail": jour[-3:],
        "evidence_pointer_count": len(ptrs or []),
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
    """회수된 서브 미러를 메인 인스턴스에 편입한다(2026-09-08 · plan_26090813 D13).

    왜 여기인가: 종전에는 서브가 완주해도 메인 인스턴스의 `phases/sub/*` 를 **사람이** 적었고,
    그래서 4개가 캠페인 종료 뒤 같은 초에 나타났다. 회수 스크립트의 종료부가 부르면 그 재저작이
    사라진다 — 회수분은 서브 저작(`authored_by = <서브 node_id>`)으로 남고, P5 가 그것을 본다.

    상향 회수는 **문서기반**이다(헌법 노드제어 ①). 이 함수가 읽는 것은 서브 디스크가 아니라
    `fetch_sub_docs.sh` 가 만든 미러뿐이며, 미러 밖은 보지 않는다.
    """
    wrote: list[str] = []
    warnings: list[str] = []
    briefs = sorted(mirror.glob("logs/*/campaign_brief.json"))
    if not briefs:
        raise WriterRefusal(
            f"회수 미러에 브리핑이 없다: {_rel(mirror)}/logs/*/campaign_brief.json — 서브가 "
            f"`campaign_init --write-brief` 를 돌지 않았거나 회수 범위가 docs/logs 를 뺐다. "
            f"부재를 통과시키면 '서브가 안 돌았다'와 '관측면이 안 왔다'가 구분되지 않는다.")
    for bpath in briefs:
        doc = _read_json(bpath)
        if not isinstance(doc, dict):
            raise WriterRefusal(f"브리핑 파손: {_rel(bpath)}")
        node = doc.get("node_id")
        if not isinstance(node, str) or not node:
            raise WriterRefusal(f"브리핑에 node_id 가 없다: {_rel(bpath)}")
        if doc.get("campaign_id") and doc["campaign_id"] != base.name:
            # 다른 캠페인의 브리핑을 편입하면 이 인스턴스의 진행표가 남의 사실을 주장한다.
            raise WriterRefusal(
                f"브리핑의 campaign_id({doc['campaign_id']!r})가 이 인스턴스({base.name!r})와 "
                f"다르다 — 회수 미러가 낡았거나 캠페인이 바뀌었다")
        for phase, st in (doc.get("phases") or {}).items():
            if not isinstance(st, dict) or phase not in PHASE_NAMES:
                continue
            wrote.append(_rel(writer_set_phase(
                base, node=node, phase=phase, state=st.get("state") or "pending",
                predicate=f"서브 회수(brief {doc.get('generated_utc')})",
                ok=bool(st.get("proof_ok")),
                source=st.get("proof_source") or f"{_rel(bpath)}#phases.{phase}",
                cell_id=None, started_utc=st.get("first_started_utc") or st.get("started_utc"),
                ended_utc=st.get("ended_utc"), authored_by=node)))
        for cell in (doc.get("cells") or []):
            if not isinstance(cell, dict) or not cell.get("cell_id"):
                continue
            outcome = cell.get("cell_outcome")
            if outcome not in CELL_OUTCOMES:
                continue                 # 서브가 아직 안 적은 셀 — 부재는 결손이지 오류가 아니다
            tps = cell.get("decode_tps_conc1")
            cpath, jpath = writer_set_cell(
                base, cell=cell["cell_id"], outcome=outcome, node=node,
                version=None, model=None,
                decode_tps=(str(tps) if isinstance(tps, (int, float)) else None),
                measurement_source=(f"{_rel(bpath)}#cells[{cell['cell_id']}]"
                                    if isinstance(tps, (int, float)) else None),
                void_reason=cell.get("void_reason"),
                void_reason_source=(f"{_rel(bpath)}#cells[{cell['cell_id']}].void_reason"
                                    if cell.get("void_reason") else None),
                axis_citation=cell.get("axis_citation"), next_intent=None, utc=utc)
            wrote.append(_rel(cpath))
            if jpath is not None:
                wrote.append(_rel(jpath))
        # 여정은 append-only 라 재편입이 줄을 늘린다 — 이미 있는 줄은 다시 적지 않는다(멱등).
        # 대조 키에서 **편입 메타(언제 가져왔나)는 뺀다** — 그것이 다르다고 같은 여정이 두 번
        # 적히면, 두 번째 줄은 새로운 사실이 아니라 회수를 한 번 더 돌렸다는 사실일 뿐이다.
        def _jkey(e: dict) -> str:
            return json.dumps({k: v for k, v in e.items()
                               if k not in ("imported_from", "imported_utc")},
                              ensure_ascii=False, sort_keys=True)

        have = {_jkey(e) for e in read_journey(base) if isinstance(e, dict)}
        for entry in (doc.get("journey_tail") or []):
            if not isinstance(entry, dict) or _jkey(entry) in have:
                continue
            have.add(_jkey(entry))
            wrote.append(_rel(writer_append_journey(
                base, dict(entry, imported_from=_rel(bpath), imported_utc=utc))))
    # 증거: 미러에 도착한 문서를 그 노드 태그로 등재한다. 종전에는 손으로 적었고 6건 전부
    # node_id 가 null 이었다 — 그래서 P2 가 아무 노드도 검사하지 못했다.
    node_ids = sorted({(_read_json(b) or {}).get("node_id") for b in briefs} - {None})
    tag = node_ids[0] if len(node_ids) == 1 else None
    for prefix, suffix, kind in _EVIDENCE_PREFIX:
        head, _, name = prefix.partition("/")
        for f in sorted((mirror / head).glob(f"{name}*{suffix}")) if (mirror / head).is_dir() else []:
            try:
                rel = str(f.resolve().relative_to(REPO_ROOT))
            except ValueError:
                continue
            try:
                wrote.append(_rel(writer_add_evidence(base, kind=kind, path_rel=rel, cell_id=None,
                                                      node=tag, unfreeze=unfreeze)))
            except WriterRefusal as exc:
                # ★ 진행표는 이미 적혔다. 증거 등재만 거부됐다면 **거기서 멈추지 않는다** —
                #   결손을 기재하고 진행한다(사용자 결정 D17: 결정론이 과하면 캠페인이 hang 한다).
                #   대개는 메인이 이미 스냅샷을 동결한 뒤에 서브가 끝난 경우이고, 그때 필요한 것은
                #   차단이 아니라 "사람이 --unfreeze 를 붙일지" 라는 질문이다.
                warnings.append(f"증거 미등재 {rel} (kind={kind}) — {exc}")
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
LIBRARY_ANSWERS = ".claude/skills/wiki-desk/fixtures/project_init_answers.yaml"
WIKI_ROOT_NAME = "__llm-wiki"


def warm_start_library(repo_root: Path | None = None) -> str:
    """질의 전에 서가를 증분 입고한다(C4). 정지한 서가에 물으면 **없다는 답이 거짓**이 된다.

    2026-09-08 실측: 서가가 09-03 에 멈춰 있어 캠페인 문서 147건이 미입고였고, 그 상태의 사서는
    "관련 자료 없음" 이라고 답했을 것이다 — 자료는 실재했다. 발행기·publish 위상·질의 진입이
    모두 이 함수를 부른다(호출부 N · 실행자를 두지 않으면 그 절차는 권고문이다).
    """
    root = REPO_ROOT if repo_root is None else Path(repo_root)
    script = root / ".claude/skills/wiki-desk/scripts/init_wiki_desk.py"
    answers = root / LIBRARY_ANSWERS
    if not (script.is_file() and answers.is_file()):
        return f"입고 건너뜀 — 사서 도구 부재({_rel(script)})"
    import subprocess
    cp = subprocess.run([sys.executable, str(script), "--project-root", str(root),
                         "--wiki-root", str(root / WIKI_ROOT_NAME), "--answers", str(answers),
                         "--incremental"], capture_output=True, text=True, cwd=str(root),
                        check=False)
    if cp.returncode != 0:
        return f"입고 실패(rc={cp.returncode}): {(cp.stderr or '').strip()[-200:]}"
    try:
        delta = (json.loads(cp.stdout) or {}).get("delta") or {}
    except ValueError:
        return "입고 완료(출력 판독 불가)"
    return (f"입고 완료 — added={delta.get('added')} changed={delta.get('changed')} "
            f"archived={delta.get('archived')}")


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

    모양은 §2.7.8 교환 스키마와 같다 — request(무엇을 찾는가) · export(사서 회신) ·
    attestation(그 회신을 어떻게 썼는가). 서브의 교환과 같은 모양이라 판정기가 하나다.
    """
    decl = read_declaration(base)
    terms = terms or grounding_terms(decl)
    if not terms:
        raise WriterRefusal(
            "통제변인에서 질의어를 파생하지 못했다 — 선언이 아직 빈칸이면 그라운딩은 "
            "선언을 채운 뒤에 한다(진입 백스톱이 그때까지 서빙을 막는다).")
    note = warm_start_library() if warm_start else "입고 생략(호출부 지시)"
    import importlib.util
    lr_path = Path(__file__).resolve().parent / "library_relay.py"
    request = {"schema_version": 1, "kind": "library.resolution.request",
               "exchange_id": f"{base.name}-{utc}", "node_id": "main", "topology": topology,
               "query": {"terms": terms},
               "purpose": "캠페인 착수 그라운딩 — 이 통제변인으로 참고할 내부 자료가 있는가"}
    if lr_path.is_file():
        spec = importlib.util.spec_from_file_location("_ground_library_relay", lr_path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        export = mod.resolve(request)
    else:
        export = {"resolution": {"status": "unresolved", "librarian": "wiki-desk",
                                 "reason": f"채널 부재: {_rel(lr_path)}"}, "references": []}
    status = ((export.get("resolution") or {}).get("status"))
    doc = {
        "schema_version": 1, "campaign_id": base.name, "asked_utc": utc,
        "library_intake": note,
        "request": request, "export": export,
        "attestation": {
            "kind": "library.resolution.attestation", "exchange_id": request["exchange_id"],
            "node_id": "main", "status": status,
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
            {"kind": "bench_report", "path": "README.md", "cell_id": None, "node_id": None}]})
        writer_add_evidence(camp, kind="bench_report", path_rel="README.md", cell_id="c1",
                            node="main", unfreeze=False)
        ck("★비어 있는 노드 태그는 정식 경로로 채운다(손편집 대체)",
           _read_json(camp / "evidence_pointers.json")["pointers"][0]["node_id"] == "main")
        ck("★음성대조 이미 다른 노드로 귀속된 증거는 조용히 바꾸지 않는다",
           _boom(lambda: writer_add_evidence(camp, kind="bench_report", path_rel="README.md",
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
        writer_freeze_evidence(camp, "2026-01-01T02:00:00Z")
        ck("★음성대조 동결 뒤 추가는 거부(입력 통로가 흐르면 태그는 불변인데 근거가 움직인다)",
           _boom(lambda: writer_add_evidence(camp, kind="devlog", path_rel="README.md",
                                             cell_id=None, node=None, unfreeze=False)))
        writer_add_evidence(camp, kind="devlog", path_rel="README.md", cell_id=None,
                            node="main", unfreeze=True)
        ck("사람이 --unfreeze 를 붙이면 통과",
           any(x.get("kind") == "devlog" and x.get("path") == "README.md"
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
           _boom(lambda: writer_add_evidence(camp, kind="sweep_map", path_rel="README.md",
                                             cell_id=None, node=None, unfreeze=True)))
        writer_add_evidence(camp, kind="sweep_map", path_rel="README.md", cell_id="c1",
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
        ck("★hint_targets 는 파생에 실리지 않는다(발행은 메인 소관)", _sl["hint_targets"] == [])
        ck("★음성대조 선언에 없는 노드는 자를 수 없다",
           _boom(lambda: emit_slice(camp, "ghost", utc="t")))
        _y3 = dict(_y2, assignments={"main": [{"cell": "c1"}]})
        _write_json(camp / "campaign.yaml", _y3)
        ck("★음성대조 배정이 빈 노드의 지시서는 거부(보낼 배정이 없는 지시서는 지시서가 아니다)",
           _boom(lambda: emit_slice(camp, "sub", utc="t")))
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
            "last_utc": "2026-09-08T22:00:00Z"})
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
        _ptrs = _read_json(camp / "evidence_pointers.json")["pointers"]
        ck("★회수 문서가 **노드 태그와 함께** 증거로 등재된다(P2 공허 통과 방지)",
           any(x["kind"] == "testlog" and x["node_id"] == "sub" for x in _ptrs)
           and any(x["kind"] == "bench_report" and x["node_id"] == "sub" for x in _ptrs))
        ck("★규약 밖 이름은 증거로 등재하지 않는다(이름이 곧 증거 연결)",
           not any("notes.md" in str(x.get("path")) for x in _ptrs))
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
    ap.add_argument("--residue-scan", action="store_true")
    ap.add_argument("--init", metavar="CAMP_ID", help="새 캠페인 개설")
    ap.add_argument("--plan-ref", help="--init 의 승인 plan 경로(필수)")
    # 반복 지정 가능(2026-09-06). 설계 의도는 "활성 인스턴스 1개"지만 현실은 그렇지 않을 수
    # 있다 — 중단된 캠페인·루트 누출 흡수분이 인스턴스를 둘 이상 남긴다. 단일 id 만 받으면
    # 남은 하나를 지울 정식 경로가 없어져 rm -rf 우회를 부른다(D3: 우회 대신 경로를 고친다).
    ap.add_argument("--purge-previous", metavar="PREV_ID", action="append", default=[],
                    help="--init 과 함께: 먼저 지울 직전 인스턴스(반복 지정 가능)")
    ap.add_argument("--from-slice", metavar="PATH",
                    help="--init 과 함께: 메인이 보낸 파생 선언으로 campaign.yaml 을 연다"
                         "(서브는 지시서만으로 착수한다)")
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

    # ── 단일 writer (2026-09-07 · plan_26090715 §4.1). ACTIVE=_bootstrap 이면 전부 no-op.
    w = ap.add_argument_group("writer (campaigns/<id>/ 에 바이트를 쓰는 유일한 자리)")
    w.add_argument("--phase-set", metavar="PHASE",
                   help="phase 상태 기록 (build|serve|bench|publish) · --node 필수")
    w.add_argument("--node", help="--phase-set 의 노드 id(= manifest role 슬러그)")
    w.add_argument("--state", help="--phase-set 의 상태 (pending|running|done|failed)")
    w.add_argument("--proof-predicate", help="무엇을 물었는가")
    w.add_argument("--proof-ok", action="store_true", help="관측 결과 참")
    w.add_argument("--proof-source", help="그 판정을 낸 명령·파일 (proof-ok 에는 필수)")
    w.add_argument("--started-utc"); w.add_argument("--ended-utc")
    w.add_argument("--cell-set", metavar="CELL", help="셀 상태 기록 · --outcome 필수")
    w.add_argument("--outcome", help="pending|measured|serve_failed|build_failed|void")
    w.add_argument("--version"); w.add_argument("--model")
    w.add_argument("--measurement-decode-tps"); w.add_argument("--measurement-source")
    w.add_argument("--sweep-state", metavar="PATH",
                   help="--cell-set 이 이 sweep 기록에서 측정값을 직접 읽는다 "
                        "(호출부가 값을 나르지 않는다 · 없으면 결손 기재)")
    w.add_argument("--authored-by", metavar="NODE",
                   help="--phase-set: 이 진행표를 적은 주체(서브 자기저작 vs 메인 사후 재저작 판별)")
    w.add_argument("--void-reason"); w.add_argument("--void-reason-source")
    w.add_argument("--axis-citation", help="이 셀이 움직인 축의 근거(layer-2 자율)")
    w.add_argument("--next-intent", help="여정 한 줄 — 다음에 무엇을 할 참인가")
    w.add_argument("--evidence-add", action="store_true", help="증거 포인터 추가 · --kind --path 필수")
    w.add_argument("--kind", help=f"증거 종류 {EVIDENCE_KINDS}")
    w.add_argument("--path", help="저장소 상대경로(실재해야 한다)")
    w.add_argument("--unfreeze", action="store_true", help="동결된 스냅샷에 사람이 명시로 추가")
    w.add_argument("--evidence-prune-stubs", action="store_true",
                   help="뼈대에서 딸려온 <<FILL>> 스텁 포인터를 정식 경로로 제거한다 "
                        "(값이 든 포인터는 건드리지 않는다 · 손삭제 대체)")
    w.add_argument("--freeze-evidence", action="store_true",
                   help="publish proof.ok 시점에 증거 스냅샷을 동결한다 · --utc 필수")
    w.add_argument("--revise", action="store_true",
                   help="layer-1 control_variables append-only 개정(메인 단일 창구)")
    w.add_argument("--revise-set", action="append", default=[], metavar="K=V",
                   help="개정할 키=값(반복)")
    w.add_argument("--revise-reason"); w.add_argument("--revise-evidence")
    w.add_argument("--revise-approved-by")
    w.add_argument("--utc", help="시각은 주입만 받는다(벽시계 금지)")
    w.add_argument("--resume-brief", action="store_true",
                   help="인스턴스만 읽고 재개 요약을 낸다(README 읽기 순서 0번)")
    a = ap.parse_args(argv)
    try:
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
        writer_ops = (a.phase_set, a.cell_set, a.evidence_add, a.freeze_evidence, a.revise,
                      a.evidence_prune_stubs, a.import_sub, a.backfill_from_docs, a.ground)
        if any(writer_ops):
            tgt = _writer_target(a.campaign_id)
            if tgt is None:
                print("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)",
                      file=sys.stderr)
                return 0
            base, camp = tgt
            wrote: list[str] = []
            if a.phase_set:
                if not a.node or not a.state:
                    raise WriterRefusal("--phase-set 은 --node 와 --state 가 필요하다")
                if a.phase_set == "publish" and a.state == "done":
                    # C4 — 발행이 끝난 그 자리에서 서가에 넣는다. 발행과 입고가 갈라지면 서가는
                    # 늘 한 캠페인만큼 늦고, 그 늦은 서가의 "없음" 은 거짓이다.
                    print(f"[campaign_init] 서가 입고(C4) — {warm_start_library()}", file=sys.stderr)
                wrote.append(_rel(writer_set_phase(
                    base, node=a.node, phase=a.phase_set, state=a.state,
                    predicate=a.proof_predicate, ok=a.proof_ok, source=a.proof_source,
                    cell_id=a.cell, started_utc=a.started_utc, ended_utc=a.ended_utc,
                    authored_by=a.authored_by)))
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
                    sweep_state=a.sweep_state)
                wrote.append(_rel(cpath))
                if jpath is not None:
                    wrote.append(_rel(jpath))
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
                _topo = (read_declaration(base).get("control_variables") or {}).get("topology")
                _gp, _gd = ground_campaign(
                    base, utc=a.utc, topology=(_topo if _topo in ("single", "multi") else "single"),
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
        if a.init:
            if not a.plan_ref:
                print("[campaign_init] FAIL --init 은 --plan-ref 가 필요하다(승인 없는 개설 ✗)", file=sys.stderr)
                return 2
            removed = []
            # 게이트는 **인스턴스마다** 따로 묻는다 — 하나가 열렸다고 다른 하나가 열리지 않는다.
            for _prev in a.purge_previous:
                removed.extend(do_purge(_prev, apply=a.apply))
            # `_bootstrap` 대기실도 함께 비운다(사용자 결정 2026-09-07). 게이트 대상이 아니므로
            # 여기서 정리하지 않으면 영원히 남는다.
            swept = sweep_bootstrap_relay(apply=a.apply)
            made = scaffold(a.init, a.plan_ref, apply=a.apply, from_slice=a.from_slice)
            mode = "APPLIED" if a.apply else "DRY-RUN"
            print(f"[campaign_init] {mode} purge={removed} init={made}")
            if swept:
                print(f"[campaign_init] {mode} _bootstrap/relay 정리 {len(swept)}건:")
                for _s in swept:
                    print(f"  - {_s}")
            return 0
        ap.print_help(); return 2
    except (PurgeGateRefusal, WriterRefusal) as exc:
        print(f"[campaign_init] FAIL {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
