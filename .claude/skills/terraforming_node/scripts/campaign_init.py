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
                     started_utc: str | None, ended_utc: str | None) -> Path:
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
    if ended_utc:
        doc["ended_utc"] = ended_utc
    doc.setdefault("started_utc", None)
    doc.setdefault("ended_utc", None)
    doc.setdefault("cell_id", None)
    _write_json(path, doc)
    return path


def writer_set_cell(base: Path, *, cell: str, outcome: str, node: str | None,
                    version: str | None, model: str | None,
                    decode_tps: str | None, measurement_source: str | None,
                    void_reason: str | None, void_reason_source: str | None,
                    axis_citation: str | None, next_intent: str | None,
                    utc: str | None) -> "tuple[Path, Path | None]":
    if outcome not in CELL_OUTCOMES:
        raise WriterRefusal(f"cell_outcome 은 {CELL_OUTCOMES} 중 하나여야 한다: {outcome!r}")
    if void_reason and not void_reason_source:
        raise WriterRefusal("--void-reason 에는 --void-reason-source 가 필수다(출처 없는 판정 ✗)")
    _no_fill(cell=cell, node=node or "", version=version or "", model=model or "")
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
    doc["measurement"] = meas
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
            return ep      # 멱등 — 같은 포인터를 두 번 적지 않는다
    pointers.append({"kind": kind, "path": path_rel, "cell_id": cell_id, "node_id": node})
    doc["pointers"] = pointers
    doc.setdefault("campaign_id", base.name)
    _write_json(ep, doc)
    return ep


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


def _order_of(doc: dict) -> list:
    order = doc.get("order")
    return [x for x in (order or []) if isinstance(x, str) and FILL not in x]


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

    order = _order_of(doc)
    cells_dir = base / "cells"
    found = sorted(p.name for p in cells_dir.iterdir()
                   if p.is_dir() and p.name != "_cell") if cells_dir.is_dir() else []
    listed = order + [c for c in found if c not in order]
    A("")
    A("## 2. 셀 진행표 (order 순)")
    pending, refuted = [], []
    for cell in listed:
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
        A(f"- `{cell}` — {outcome}" + (f"  ({' · '.join(extra)})" if extra else ""))
        # 상태 파일이 아예 없는 셀도 **남은 작업**이다 — "돌지 않았다"와 "종결했다"를 같은 값으로
        # 접으면 전 셀 미개설인 새 캠페인이 '완주' 로 보인다(부재 ≠ 통과).
        if outcome in ("pending", "(상태파일 없음)", "(미개설)"):
            pending.append(cell)
        if outcome in ("serve_failed", "build_failed", "void"):
            refuted.append((cell, cite, st.get("void_reason")))
    if not listed:
        A("- (셀 없음)")

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
        # ── 단일 writer + resume-brief (2026-09-07 · plan_26090715 §4.1·§4.4) ──────────────
        camp = CAMPAIGNS / "w1"
        for sub in ("cells", "phases", "sweeps", "relay"):
            (camp / sub).mkdir(parents=True, exist_ok=True)
        (camp / "campaign.yaml").write_text(json.dumps({
            "schema_version": 1, "id": "w1", "plan_ref": "docs/plan/p.md",
            "declared_utc": "2026-01-01T00:00:00Z",
            "nodes": [{"node_id": "main", "role": "main"}, {"node_id": "sub", "role": "sub"}],
            "order": ["c1", "c2"],
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
        ck("★음성대조 실재하지 않는 증거 거부",
           _boom(lambda: writer_add_evidence(camp, kind="testlog", path_rel="does/not/exist.md",
                                             cell_id=None, node=None, unfreeze=False)))
        ck("★음성대조 알 수 없는 kind 거부",
           _boom(lambda: writer_add_evidence(camp, kind="nope", path_rel="CLAUDE.md",
                                             cell_id=None, node=None, unfreeze=False)))
        writer_freeze_evidence(camp, "2026-01-01T02:00:00Z")
        ck("★음성대조 동결 뒤 추가는 거부(입력 통로가 흐르면 태그는 불변인데 근거가 움직인다)",
           _boom(lambda: writer_add_evidence(camp, kind="devlog", path_rel="README.md",
                                             cell_id=None, node=None, unfreeze=False)))
        writer_add_evidence(camp, kind="devlog", path_rel="README.md", cell_id=None,
                            node=None, unfreeze=True)
        ck("사람이 --unfreeze 를 붙이면 통과", len(_read_json(camp / "evidence_pointers.json")["pointers"]) == 2)

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
    w.add_argument("--void-reason"); w.add_argument("--void-reason-source")
    w.add_argument("--axis-citation", help="이 셀이 움직인 축의 근거(layer-2 자율)")
    w.add_argument("--next-intent", help="여정 한 줄 — 다음에 무엇을 할 참인가")
    w.add_argument("--evidence-add", action="store_true", help="증거 포인터 추가 · --kind --path 필수")
    w.add_argument("--kind", help=f"증거 종류 {EVIDENCE_KINDS}")
    w.add_argument("--path", help="저장소 상대경로(실재해야 한다)")
    w.add_argument("--unfreeze", action="store_true", help="동결된 스냅샷에 사람이 명시로 추가")
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
        writer_ops = (a.phase_set, a.cell_set, a.evidence_add, a.freeze_evidence, a.revise)
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
                wrote.append(_rel(writer_set_phase(
                    base, node=a.node, phase=a.phase_set, state=a.state,
                    predicate=a.proof_predicate, ok=a.proof_ok, source=a.proof_source,
                    cell_id=a.cell, started_utc=a.started_utc, ended_utc=a.ended_utc)))
            if a.cell_set:
                if not a.outcome:
                    raise WriterRefusal("--cell-set 은 --outcome 이 필요하다")
                cpath, jpath = writer_set_cell(
                    base, cell=a.cell_set, outcome=a.outcome, node=a.node,
                    version=a.version, model=a.model,
                    decode_tps=a.measurement_decode_tps,
                    measurement_source=a.measurement_source,
                    void_reason=a.void_reason, void_reason_source=a.void_reason_source,
                    axis_citation=a.axis_citation, next_intent=a.next_intent, utc=a.utc)
                wrote.append(_rel(cpath))
                if jpath is not None:
                    wrote.append(_rel(jpath))
            if a.evidence_add:
                if not a.kind or not a.path:
                    raise WriterRefusal("--evidence-add 는 --kind 와 --path 가 필요하다")
                wrote.append(_rel(writer_add_evidence(
                    base, kind=a.kind, path_rel=a.path, cell_id=a.cell, node=a.node,
                    unfreeze=a.unfreeze)))
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
    except (PurgeGateRefusal, WriterRefusal) as exc:
        print(f"[campaign_init] FAIL {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
