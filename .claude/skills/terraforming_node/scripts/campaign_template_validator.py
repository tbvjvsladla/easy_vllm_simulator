#!/usr/bin/env python3
"""campaign_template_validator.py — 캠페인 선언·인스턴스의 결정론 검증기 (fail-closed).

왜 있는가(plan_26090616): 캠페인의 단계 사이에서 정보를 나르던 것이 **대화 기억**이었다. 세션이
끊기거나 문맥이 압축되면 앞 단계의 결정이 사라졌고, 각 스킬은 자기 기본값(루트 `config.yaml`·루트
`tasks/`)으로 되돌아가 산출물을 관리범위 밖에 흘렸다. 처방은 나를 것을 파일로 만드는 것이고,
이 파일은 **그 파일이 실제로 채워졌는지**를 묻는다.

무엇을 검사하지 않는가: 해시. 뼈대는 추적물이라 git 이 이미 바이트를 들고, 인스턴스는 휘발이라
대조할 두 번째 자리가 없다(`policy:GIT_SINGLE_AUTHORITY` 2문항). 사용자 결정 2026-09-06:
"SHA256 같은 무결성 검증은 필요 없다".

  검증:  campaign_template_validator.py --campaign campaigns/<id>/campaign.yaml
  뼈대:  campaign_template_validator.py --template
  단위:  campaign_template_validator.py --selftest
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
CAMPAIGNS = REPO_ROOT / "campaigns"
TEMPLATE = CAMPAIGNS / "_template"
SCHEMA_REL = "campaigns/_template/campaign.schema.json"
FILL = "<<FILL>>"
RESERVED_IDS = ("_template", "_bootstrap")
# 스캐폴드가 남기는 **틀** 디렉터리. 실행 순서(order)의 시민이 아니다.
RESERVED_CELL_NAMES = ("_cell", "_node")
# 셀 전이 모드(2026-09-08 · plan_26090813 D11). 생략은 AUTO 이고, STAY 는 리스트 마지막에만 온다.
ASSIGNMENT_MODES = ("AUTO", "HITL", "STAY")
DEFAULT_MODE = "AUTO"


class CampaignContractFailure(Exception):
    """검증 실패. 호출부는 rc!=0 으로 옮긴다 — 삼키지 않는다."""


def _rel(path: Path) -> str:
    """저장소 안이면 상대경로로, 밖(픽스처·서브 트리)이면 그대로. 경로 하나 때문에 검증기가
    죽으면 그 순간 검증이 없는 것과 같다 — 표시 편의가 판정을 막지 않게 한다."""
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def _die(msg: str) -> None:
    raise CampaignContractFailure(msg)


def _load_json(path: Path, what: str) -> object:
    if not path.is_file():
        _die(f"{what} 부재: {path}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        _die(f"{what} 파손({path}): {exc}")


def _hint_tag_module():
    """arch 문법의 **단일 소유자**는 발행기다. 여기서 정규식을 복제하면 두 자리가 갈라지고,
    갈라진 쪽이 조용히 늦는다(workflow.md §결정론 규율 — 개념 중복)."""
    path = REPO_ROOT / ".claude/skills/hint-publisher/scripts/hint_tag.py"
    if not path.is_file():
        return None
    spec = importlib.util.spec_from_file_location("_campaign_hint_tag", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _validate_schema(doc: object, schema: dict, where: str) -> list[str]:
    """completion_gate 가 이미 구현한 Draft-07 부분집합을 그대로 쓴다(두 번째 엔진을 만들지 않는다).

    ★ 서브 오버레이에는 이 엔진이 배달되지 않는다(`.claude/policies/` 는 배달 표면 밖이다).
      부재를 조용히 통과시키지 않는다 — 스키마를 안 본 통과는 통과가 아니다. 대신 **무엇이 없고
      누가 그 검증을 소유하는지**를 말한다: 선언 검증은 메인 단일 창구이고, 서브는 메인이 검증해
      보낸 파생 선언(`--from-slice`)을 소비하며 자기 인스턴스는 P1~P3 으로 본다.
    """
    path = REPO_ROOT / ".claude/policies/runtime/completion_gate.py"
    if not path.is_file():
        _die(f"스키마 엔진이 없다: {_rel(path)} — 이 트리에는 검증기만 있고 엔진이 오지 않았다. "
             f"선언 검증은 메인 단일 창구이며(서브는 --from-slice 로 받은 선언을 소비한다), "
             f"이 트리에서 물을 수 있는 것은 --instance 의 P1~P3 이다.")
    spec = importlib.util.spec_from_file_location("_campaign_completion_gate", path)
    gate = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gate)
    return [f"{where}: {v}" for v in gate.validate_against_schema(doc, schema)]


def find_fill_placeholders(text: str) -> list[int]:
    """`<<FILL>>` 이 남은 행 번호. 모르는 값을 그럴듯하게 채우는 것보다 **모른다고 멈추는** 것이 싸다."""
    return [i for i, line in enumerate(text.splitlines(), 1) if FILL in line]


def strip_annotations(doc: dict) -> dict:
    """`_` 로 시작하는 **최상위** 키는 사람 안내문이다 — 스키마 대조·빈칸 스캔의 시민이 아니다.

    왜 면제하는가(2026-09-08 · plan_26090813 D10): 안내문이 자기 스캔에 걸리면 사람이 그것을
    지운다. 그리고 지운 흔적은 "채워야 했는데 못 채워서 지운 빈칸"과 구분되지 않는다 —
    camp-26090721 사후감사에서 손삭제 2건이 정확히 그 모양이었다. docs.md 가 "규칙 문서가 자기
    스캔에 걸리면 게이트가 무의미해진다"고 적은 것과 같은 처방이며, 처방은 삭제가 아니라 면제다.

    최상위만 면제한다 — 깊은 자리의 `_enum` 류는 값의 일부라 면제하면 빈칸이 거기 숨는다.
    """
    return {k: v for k, v in doc.items() if not (isinstance(k, str) and k.startswith("_"))}


def find_fill_paths(node, path: str = "") -> list[str]:
    """`<<FILL>>` 이 남은 **자리**(JSON 경로). 행 번호가 아니라 자리를 돌려주는 이유: 안내문을
    면제하려면 구조를 알아야 하는데 행 스캔은 구조를 모른다."""
    out: list[str] = []
    if isinstance(node, dict):
        for k, v in node.items():
            here = f"{path}.{k}" if path else str(k)
            if isinstance(k, str) and FILL in k:
                out.append(f"{here} (키)")
            out += find_fill_paths(v, here)
    elif isinstance(node, list):
        for i, v in enumerate(node):
            out += find_fill_paths(v, f"{path}[{i}]")
    elif isinstance(node, str) and FILL in node:
        out.append(path or "$")
    return out


def scan_fill_file(path: Path) -> list[str]:
    """파일 하나의 빈칸 자리. `.json` 은 구조로(최상위 안내문 면제), 그 밖은 행으로 훑는다 —
    YAML·셸에는 파서가 없으므로 행이 최선이고, 거기엔 면제할 '안내문 키'라는 개념도 없다."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"(읽기 실패: {exc})"]
    if path.suffix == ".json":
        try:
            doc = json.loads(text)
        except ValueError:
            return [f"{i}행" for i in find_fill_placeholders(text)]
        if isinstance(doc, dict):
            doc = strip_annotations(doc)
        return find_fill_paths(doc)
    return [f"{i}행" for i in find_fill_placeholders(text)]


def assignments_of(doc: dict) -> dict:
    """노드별 배정. **배정의 단일 소유자**는 이 함수이고, 읽는 쪽(campaign_init·relay)은 여기를
    부른다 — 두 자리에 파싱을 적으면 한쪽이 조용히 늦는다."""
    a = doc.get("assignments")
    return a if isinstance(a, dict) else {}


def assignment_items(doc: dict, node: str) -> list:
    items = assignments_of(doc).get(node)
    return [x for x in items if isinstance(x, dict)] if isinstance(items, list) else []


def assigned_cells(doc: dict, node: str | None = None) -> list[str]:
    """실행 대상 셀. `node` 를 주면 그 노드 몫, 없으면 선언 순서대로 평탄화한 전체."""
    out: list[str] = []
    for nid, items in assignments_of(doc).items():
        if node is not None and nid != node:
            continue
        for item in (items if isinstance(items, list) else []):
            cell = item.get("cell") if isinstance(item, dict) else None
            if isinstance(cell, str) and cell and FILL not in cell and cell not in out:
                out.append(cell)
    return out


def cell_mode(doc: dict, node: str, cell: str) -> str:
    """셀 종료 뒤의 전이. 생략은 AUTO 다 — 기본값을 '없음'으로 두면 호출부마다 다른 기본이 생긴다."""
    for item in assignment_items(doc, node):
        if item.get("cell") == cell:
            mode = item.get("mode")
            return mode if mode in ASSIGNMENT_MODES else DEFAULT_MODE
    return DEFAULT_MODE


def validate_campaign(campaign_path: Path, *, strict_paths: bool = True) -> list[str]:
    """캠페인 선언 1건을 검증하고 **위반 목록**을 돌려준다(빈 리스트 = 통과)."""
    problems: list[str] = []
    raw = campaign_path.read_text(encoding="utf-8") if campaign_path.is_file() else None
    if raw is None:
        return [f"campaign 선언 부재: {campaign_path}"]

    try:
        doc = json.loads(raw)
    except ValueError as exc:
        rows = find_fill_placeholders(raw)
        return ([f"{campaign_path.name}: JSON 파손 — {exc}"]
                + ([f"{campaign_path.name}: 플레이스홀더 {FILL} 가 {len(rows)}행에 남아 있다"
                    f"(행 {rows[:10]})"] if rows else []))
    if not isinstance(doc, dict):
        return [f"{campaign_path.name}: 최상위가 객체가 아니다"]

    # 안내문(`_` 접두 최상위 키)은 스키마 대조·빈칸 스캔 **양쪽**에서 걷어낸다. 한쪽만 걷으면
    # 남은 쪽이 안내문을 결함이라고 말하고, 사람은 그것을 지워서 입을 막는다(2026-09-08 §A).
    doc = strip_annotations(doc)
    fills = find_fill_paths(doc)
    if fills:
        problems.append(f"{campaign_path.name}: 플레이스홀더 {FILL} 가 {len(fills)}곳에 남아 있다"
                        f"({', '.join(fills[:8])})")

    schema = _load_json(REPO_ROOT / SCHEMA_REL, "campaign schema")
    problems += _validate_schema(doc, schema, campaign_path.name)
    if problems:
        return problems

    camp_id = doc.get("id")
    if camp_id in RESERVED_IDS:
        problems.append(f"예약 id 는 캠페인 이름이 될 수 없다: {camp_id!r} (예약: {RESERVED_IDS})")
    if camp_id and campaign_path.parent.name not in (camp_id, "_template"):
        problems.append(f"선언 id({camp_id!r})와 디렉터리 이름({campaign_path.parent.name!r})이 다르다 "
                        f"— 이름이 곧 증거 연결이다")

    node_ids = {n.get("node_id") for n in doc.get("nodes", [])}
    if len(node_ids) != len(doc.get("nodes", [])):
        problems.append("nodes[].node_id 가 중복이다 — 노드 정체성이 갈리지 않는다")

    # budgets: 기본값 금지. 선언이 없으면 arm_ceiling 이 무한대가 되어 호스트가 무방비다.
    budgets = doc.get("budgets", {})
    for key in ("smoke_budget_overhead_mib", "ready_max_seconds"):
        if not isinstance(budgets.get(key), int) or budgets[key] <= 0:
            problems.append(f"budgets.{key} 가 선언되지 않았다(0/기본값 금지 — 선언 없으면 무방비)")

    # 통제변인: 서브에 전달하는 표의 정본. 빠지면 서브가 저장소 기본값을 집어 엉뚱한 대상을 잰다.
    for key in ("model", "vllm_version", "topology", "target_gpu"):
        if not str(doc.get("control_variables", {}).get(key, "")).strip():
            problems.append(f"control_variables.{key} 공란 — 서브가 저장소 기본값으로 되돌아간다(2026-09-05 실측)")

    # hint 대상: arch 문법은 발행기가 소유한다(여기서 복제하지 않는다).
    ht = _hint_tag_module()
    for i, target in enumerate(doc.get("hint_targets", [])):
        arch = target.get("arch", "")
        if ht is not None and (why := ht.arch_violation(arch)) is not None:
            problems.append(f"hint_targets[{i}].arch {arch!r}: {why} "
                            f"(문법 <hw>-<main|sub|cluster>-<target>)")
        nid = target.get("node_id")
        if nid not in node_ids:
            problems.append(f"hint_targets[{i}].node_id {nid!r} 가 nodes[] 에 없다")
            continue
        # 배정 SSOT 는 assignments 이고 hint_targets 는 **파생**이다. 두 자리가 갈라지면 태그가
        # 자기가 요약하지 않은 셀을 주장한다 — 이름이 곧 증거 연결이라 그 주장은 게이트를 지난다.
        assigned_here = set(assigned_cells(doc, nid))
        for c in (target.get("cells") or []):
            if isinstance(c, str) and c not in assigned_here:
                problems.append(f"hint_targets[{i}].cells 의 {c!r} 이 assignments[{nid!r}] 에 없다 "
                                f"— 배정 SSOT 는 assignments 이고 hint_targets 는 파생이다")

    # ── 배정 ↔ cells 실재 (2026-09-08 · plan_26090813 D11 — 옛 평면 `order` 의 후속) ──────
    # ★ 2026-09-06: 종전에는 config.yaml 의 **실재**만 물었다. 그런데 스캐폴드가 남기는 틀
    #   `cells/_cell/config.yaml` 도 실재하는 파일이라, 실행 목록을 디렉터리 목록에서 파생하면
    #   틀이 그대로 섞여 들어가고 검증기가 통과시켰다(내가 실제로 그렇게 했다).
    # ★ 2026-09-08: 평면 목록은 "메인이 A·B, 서브가 C·D" 를 표현하지 못했다. 표현할 수 없는
    #   것은 배선될 수 없고, 그래서 병렬 캠페인이 순차로 돌았다. 배정은 이제 노드별이다.
    cells_dir = campaign_path.parent / "cells"
    assigns = assignments_of(doc)
    if not assigns:
        problems.append("assignments 가 비었다 — 배정이 없으면 어느 노드도 돌 것이 없다")
    owner: dict = {}
    for node, items in assigns.items():
        if node not in node_ids:
            problems.append(f"assignments 의 노드 {node!r} 가 nodes[] 에 없다")
        items = items if isinstance(items, list) else []
        for i, item in enumerate(items):
            if not isinstance(item, dict):
                continue
            mode = item.get("mode", DEFAULT_MODE)
            if mode == "STAY" and i != len(items) - 1:
                problems.append(
                    f"assignments[{node!r}][{i}] 의 mode=STAY 가 마지막이 아니다 — STAY 는 벤치 뒤에도 "
                    f"서빙을 유지하므로 뒤에 남은 {len(items) - 1 - i}개 셀이 영원히 돌지 않는다")
            cell = item.get("cell")
            if not isinstance(cell, str) or not cell:
                continue
            if cell in RESERVED_CELL_NAMES:
                problems.append(f"assignments 에 틀 디렉터리 {cell!r} 가 들어 있다 — 틀은 실행 대상이 아니다")
                continue
            if cell in owner:
                problems.append(f"셀 {cell!r} 이 노드 {owner[cell]!r} 와 {node!r} 에 이중 배정됐다 "
                                f"— 셀의 수행 노드는 하나다(증거 태그가 갈린다)")
                continue
            owner[cell] = node
            cfg = cells_dir / cell / "config.yaml"
            if strict_paths and not cfg.is_file():
                problems.append(f"배정된 셀 {cell!r} 입력이 없다: {_rel(cfg)}")
                continue
            if not strict_paths:
                continue
            # 셀 입력의 빈칸도 계약이다 — 모르는 값을 그럴듯하게 채우지 말라는 규칙(campaigns/README)
            # 은 검사가 있어야 규칙이다.
            for name in ("config.yaml", "lockset.json"):
                f = cells_dir / cell / name
                if not f.is_file():
                    continue
                where = scan_fill_file(f)
                if where:
                    problems.append(f"셀 {cell!r} 의 {name} 에 {FILL} 가 {len(where)}곳 남아 있다"
                                    f"({', '.join(where[:5])})")
    return problems


def validate_instance(camp_dir: Path) -> list[str]:
    """인스턴스 하나를 검증한다 — 선언 + 셀 + phase proof."""
    problems = validate_campaign(camp_dir / "campaign.yaml")
    # ★ 증거 포인터의 빈칸도 계약이다. 종전에는 이 파일을 검증기가 아예 읽지 않아 뼈대 스텁
    #   `pointers[0]` 이 살아남았고, **검증기는 PASS 인데 purge 게이트만 닫히는** 두 판정기
    #   불일치가 났다(2026-09-08 사후감사 §A F1). 같은 사실을 두 자리가 다르게 말하면 사람은
    #   조용한 쪽을 믿고 시끄러운 쪽을 손으로 지운다.
    ep = camp_dir / "evidence_pointers.json"
    if ep.is_file():
        where = scan_fill_file(ep)
        if where:
            problems.append(f"evidence_pointers.json 에 {FILL} 가 {len(where)}곳 남아 있다"
                            f"({', '.join(where[:5])}) — 성장 배열은 빈 목록으로 출발한다")
    for status in sorted(camp_dir.glob("phases/*/*.status.json")):
        if status.parent.name in RESERVED_CELL_NAMES:
            continue                      # `_node` 는 틀이지 수행 주체가 아니다
        try:
            doc = json.loads(status.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"{status.name}: 파손 — {exc}")
            continue
        proof = doc.get("proof") or {}
        if proof.get("ok") is True and not str(proof.get("source") or "").strip():
            problems.append(f"{status.parent.name}/{status.name}: proof.ok=true 인데 source 가 비었다 "
                            f"— 출처 없는 판정은 단언이 검증을 대체한 것이다")
        if doc.get("state") == "done" and proof.get("ok") is not True:
            problems.append(f"{status.parent.name}/{status.name}: state=done 인데 proof.ok 가 참이 아니다")
    for cell_status in sorted(camp_dir.glob("cells/*/cell.status.json")):
        if cell_status.parent.name in RESERVED_CELL_NAMES:
            continue
        try:
            doc = json.loads(cell_status.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"{cell_status.name}: 파손 — {exc}")
            continue
        if doc.get("cell_outcome") == "void" and not str(doc.get("void_reason_source") or "").strip():
            problems.append(f"{cell_status.parent.name}: void 인데 void_reason_source 가 비었다")
    problems.extend(instance_predicates(camp_dir))
    return problems


# ─────────────────────────────────────────────────────────────────────────────
# P1~P3 — 완주를 표현할 수 있는 술어 (2026-09-07 신설 · plan_26090715 §4.4)
#
# 왜: 종전 검증기는 `pending` 을 합법 enum 으로만 보아 **완주 후 전부-pending 이 PASS** 였다.
# 캠페인 ⑦ 이 그 상태로 통과했다 — sweep 은 b1~b6 을 serve_failed/measured 로 기록했는데
# cell.status 는 pending 이었고, 선언된 노드 `sub`·`sub-mn` 은 phases/ 디렉터리 자체가 없었다.
# 셋 다 인스턴스 안의 데이터만으로 계산되는데 하나도 없었다(audit_26090708 §1.1).
#
# 새 lint 계열을 만들지 않는다 — 이 셋은 purge 선행조건에 편입되는 **기존 게이트의 술어**다.

# sweep 레코드의 결과 어휘 → cell_outcome 어휘. 두 자리가 다른 말을 쓰면 대조가 성립하지 않는다.
# ★ 2026-09-08 라이브 교정: `classify_cell.py` 가 내는 `measurement_void`·`not_measured` 가 이 표에
#   없어서 P1 은 그 셀들을 **아무 말 없이 건너뛰었다**(부재를 불일치로 세지 않는 설계가, 어휘가
#   갈라진 순간 침묵 통과가 됐다). 두 자리가 다른 말을 쓰면 대조가 성립하지 않는다.
_SWEEP_TO_CELL = {
    "measured": "measured", "serve_failed": "serve_failed", "build_failed": "build_failed",
    "void": "void", "pending": "pending",
    "measurement_void": "measurement_void", "not_measured": "not_measured",
}


def _sweep_cell_records(camp_dir: Path) -> dict:
    """sweeps/*.json 이 든 셀별 결과. 키 = cell_key, 값 = (outcome, 출처 파일)."""
    out: dict = {}
    for sweep in sorted(camp_dir.glob("sweeps/*.json")):
        try:
            doc = json.loads(sweep.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        for cell in (doc.get("cells") or []):
            if not isinstance(cell, dict):
                continue
            key = cell.get("cell_key") or cell.get("cell_id") or cell.get("config")
            outcome = cell.get("cell_outcome") or cell.get("outcome") or cell.get("status")
            if isinstance(key, str) and isinstance(outcome, str):
                out[key] = (outcome, _rel(sweep))
    return out


def predicate_p1(camp_dir: Path) -> list[str]:
    """P1 — sweep 레코드와 cell.status 가 같은 말을 하는가.

    갈라지면 재개 에이전트가 이미 돈 셀을 다시 돈다(README 는 `pending` 인 셀이 남은 작업이라고
    말한다). 부재는 결손이지만 **불일치는 차단**이다(인터뷰 Q2 절단선).
    """
    problems: list[str] = []
    sweeps = _sweep_cell_records(camp_dir)
    if not sweeps:
        return problems
    for key, (sweep_outcome, src) in sorted(sweeps.items()):
        status_path = camp_dir / "cells" / key / "cell.status.json"
        want = _SWEEP_TO_CELL.get(sweep_outcome)
        if want is None:
            continue
        if not status_path.is_file():
            problems.append(f"P1 {key}: sweep 은 '{sweep_outcome}' 인데 cell.status.json 이 없다 "
                            f"(출처 {src}) — 돌았는데 상태를 아무도 적지 않았다")
            continue
        try:
            doc = json.loads(status_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"P1 {key}: cell.status.json 파손 — {exc}")
            continue
        have = doc.get("cell_outcome")
        if have != want:
            problems.append(f"P1 {key}: sweep='{sweep_outcome}' vs cell.status='{have}' 불일치 "
                            f"(출처 {src}) — 두 자리가 갈라지면 재개가 이미 돈 셀을 다시 돈다")
    return problems


def read_declaration(camp_dir: Path) -> dict:
    """인스턴스의 선언. 읽기 전용이며 파손·부재는 빈 dict 다 — 술어가 선언 때문에 죽으면
    그 순간 술어가 없는 것과 같다(_rel 과 같은 이유)."""
    try:
        doc = json.loads((camp_dir / "campaign.yaml").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return doc if isinstance(doc, dict) else {}


def declared_node_ids(camp_dir: Path, role: str | None = None) -> list[str]:
    out: list[str] = []
    for n in (read_declaration(camp_dir).get("nodes") or []):
        if not isinstance(n, dict):
            continue
        nid = n.get("node_id")
        if not isinstance(nid, str) or not nid or FILL in nid:
            continue
        if role is not None and n.get("role") != role:
            continue
        out.append(nid)
    return out


def predicate_p2(camp_dir: Path) -> list[str]:
    """P2 — 증거가 도착한 (노드, 셀)의 phase 가 그 사실을 반영하는가.

    벤치 증거(인증서·리포트)가 실재하는데 그 노드의 bench phase 가 `done` 이 아니면, 진행표가
    증거보다 낡은 것이다. 진행표를 믿고 재개하면 이미 잰 셀을 다시 잰다.

    ★ 2026-09-08 강화(plan_26090813 §4.4): 종전 P2 는 `node_id` 가 **문자열일 때만** 물었다.
      camp-26090721 의 포인터 6건은 전부 `null` 이었고, 그래서 이 술어는 아무 노드도 검사하지
      않은 채 통과했다 — 공허 통과다. 다노드 캠페인에서 노드 태그 없는 벤치 증거는 "어느 노드가
      쟀는지 모르는 측정"이고, 그건 증거가 아니라 미기재다. 단일노드에서는 귀속이 자명하므로
      태그 부재를 결함으로 보지 않고 그 하나의 노드에 귀속시킨다(과잉차단 ✗).
    """
    problems: list[str] = []
    ep = camp_dir / "evidence_pointers.json"
    if not ep.is_file():
        return problems
    try:
        doc = json.loads(ep.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return problems
    nodes = declared_node_ids(camp_dir)
    multi = len(nodes) > 1
    bench_nodes: dict = {}
    bench_total = 0
    for ptr in (doc.get("pointers") or []):
        if not isinstance(ptr, dict):
            continue
        if str(ptr.get("kind")) in ("certificate", "bench_report"):
            bench_total += 1
            node = ptr.get("node_id")
            if isinstance(node, str) and node:
                bench_nodes.setdefault(node, []).append(str(ptr.get("path")))
            elif multi:
                problems.append(
                    f"P2 노드 태그 없는 벤치 증거: {ptr.get('path')} — {len(nodes)}노드 캠페인에서는 "
                    f"어느 노드가 잰 것인지 알 수 없다(`--evidence-add --node <id>` 누락)")
            elif len(nodes) == 1:
                bench_nodes.setdefault(nodes[0], []).append(str(ptr.get("path")))
    if multi and bench_total and not bench_nodes:
        problems.append(
            f"P2 벤치 증거 {bench_total}건이 **전부** 노드 태그가 없다 — 이 상태의 P2 는 아무 노드도 "
            f"검사하지 않고 통과한다(공허 통과 금지 · 2026-09-08 재발 방지)")
    for node, paths in sorted(bench_nodes.items()):
        status_path = camp_dir / "phases" / node / "bench.status.json"
        if not status_path.is_file():
            problems.append(f"P2 {node}: 벤치 증거 {len(paths)}건이 있는데 "
                            f"phases/{node}/bench.status.json 이 없다 — 진행표가 증거보다 낡았다")
            continue
        try:
            st = json.loads(status_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"P2 {node}: bench.status.json 파손 — {exc}")
            continue
        if st.get("state") != "done" or (st.get("proof") or {}).get("ok") is not True:
            problems.append(f"P2 {node}: 벤치 증거가 도착했는데 bench phase 가 "
                            f"state={st.get('state')!r} proof.ok={(st.get('proof') or {}).get('ok')!r} "
                            f"— 진행표가 증거보다 낡았다")
    return problems


def predicate_p3(camp_dir: Path) -> list[str]:
    """P3 — 선언된 노드 전수에 phases/ 가 있는가.

    `campaign.yaml.nodes[]` 에 적었는데 진행표가 없으면 그 노드는 "안 돌았다"와 "돌았는데 아무도
    안 적었다"가 구분되지 않는다. 부재와 실패는 다른 사실이다.
    """
    problems: list[str] = []
    doc = camp_dir / "campaign.yaml"
    if not doc.is_file():
        return problems
    try:
        decl = json.loads(doc.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return problems
    for node in (decl.get("nodes") or []):
        if not isinstance(node, dict):
            continue
        nid = node.get("node_id")
        if not isinstance(nid, str) or not nid or FILL in nid:
            continue
        if not (camp_dir / "phases" / nid).is_dir():
            problems.append(f"P3 {nid}: campaign.yaml 이 선언한 노드인데 phases/{nid}/ 가 없다 "
                            f"— '안 돌았다'와 '돌았는데 아무도 안 적었다'가 구분되지 않는다")
    return problems


def instance_predicates(camp_dir: Path) -> list[str]:
    """P1~P3 을 한 번에. purge 선행조건이 이 함수를 부른다."""
    return predicate_p1(camp_dir) + predicate_p2(camp_dir) + predicate_p3(camp_dir)


# ─────────────────────────────────────────────────────────────────────────────
# P4·P5 — 합격 술어 (2026-09-08 신설 · plan_26090813 §4.7)
#
# ★ 왜 purge 선행조건이 **아닌가**: P1~P3 은 "지워도 되는가"를 묻고, P4·P5 는 "이번 실행이
#   설계대로 돌았는가"를 묻는다. 후자를 purge 게이트에 넣으면 설계대로 못 돈 캠페인이 영원히
#   지워지지 않아 다음 캠페인이 시작조차 못 한다 — 그것이 사용자가 경계한 "무한 hang"이다
#   (plan §9 R3). 진행을 막는 자리와 판정하는 자리를 분리한다.

def _relay_ledgers(camp_dir: Path) -> list[dict]:
    """릴레이 원장. `pending_hitl.json` 은 원장이 아니라 대기표라 제외한다."""
    out: list[dict] = []
    for path in sorted(camp_dir.glob("relay/*.json")):
        if path.name == "pending_hitl.json":
            continue
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if isinstance(doc, dict):
            doc = dict(doc, _ledger_path=_rel(path))
            out.append(doc)
    return out


def _phase_docs(camp_dir: Path, node: str) -> list[tuple[str, dict]]:
    out: list[tuple[str, dict]] = []
    for path in sorted((camp_dir / "phases" / node).glob("*.status.json")):
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if isinstance(doc, dict):
            out.append((path.name, doc))
    return out


def _first_start(doc: dict) -> str | None:
    """그 phase 가 **처음** 시작한 시각. `started_utc` 는 셀마다 덮어써지므로 마지막 셀을
    가리킨다 — 착수 시각을 묻는 술어가 그걸 읽으면 늦은 값을 보고 통과한다."""
    for key in ("first_started_utc", "started_utc"):
        val = doc.get(key)
        if isinstance(val, str) and val:
            return val
    return None


def predicate_p4(camp_dir: Path) -> list[str]:
    """P4 — 서브 지시서가 메인 첫 착수보다 **먼저** 나갔는가(동시 착수의 관측 가능한 정의).

    camp-26090721 은 서브 위임이 메인 두 셀을 끝낸 뒤(18:36Z)에 나갔다. "병렬로 돌 수 있었다"는
    검증되지 않는다 — 검증되는 것은 **언제 나갔나**뿐이고, 이 술어가 그 시각을 본다
    (사용자 결정 D3: single 은 서브 지시서 먼저, 메인 셀은 그 다음).
    """
    problems: list[str] = []
    subs = declared_node_ids(camp_dir, role="sub")
    mains = declared_node_ids(camp_dir, role="main")
    if not subs or not mains:
        return problems           # 서브가 없는 캠페인에 동시 착수는 성립하지 않는다(부재 ≠ 위반)
    starts = [t for m in mains for _, d in _phase_docs(camp_dir, m) if (t := _first_start(d))]
    if not starts:
        return [f"P4: 메인({', '.join(mains)}) phase 에 시각이 하나도 없다 — 언제 착수했는지 물을 "
                f"수단이 없다(writer 호출부의 --started-utc 누락)"]
    main_start = min(starts)
    ledgers = _relay_ledgers(camp_dir)
    for sub_id in subs:
        build = [l for l in ledgers if l.get("campaign_node") == sub_id
                 and l.get("campaign_context_kind") == "build"]
        if not build:
            problems.append(
                f"P4 {sub_id}: 빌드 context 원장이 없다 — 서브 지시서 선발급의 관측 자리가 비었다"
                f"(relay 가 campaign_node·campaign_context_kind 를 원장에 적어야 한다)")
            continue
        attempts = [a.get("started_utc") for l in build for a in (l.get("attempts") or [])
                    if isinstance(a, dict) and isinstance(a.get("started_utc"), str)]
        if not attempts:
            problems.append(f"P4 {sub_id}: 빌드 context 는 열렸는데 attempt 가 0 이다 — "
                            f"원장만 있고 아무것도 나가지 않았다")
            continue
        first = min(attempts)
        if first >= main_start:
            problems.append(
                f"P4 {sub_id}: 서브 빌드 지시서({first})가 메인 첫 착수({main_start})보다 늦다 — "
                f"이것이 '순차로 돌았다'의 관측 가능한 정의다")
    return problems


def predicate_p5(camp_dir: Path, *, brief_paths: list | None = None) -> list[str]:
    """P5 — 서브의 진행표를 **서브가** 적었는가.

    camp-26090721 의 `phases/sub/*` 4개는 22:34:11Z 에 **같은 초로** 나타났다. 서브가 도는 동안
    적힌 것이 아니라 끝난 뒤 메인이 한 번에 저작한 것이고, 그러면 진행표는 관측이 아니라 회고다
    — 감독자가 그것을 읽어도 진행을 알 수 없다(그래서 메인이 ssh 로 직접 봤다).
    """
    problems: list[str] = []
    mains = set(declared_node_ids(camp_dir, role="main"))
    for sub_id in declared_node_ids(camp_dir, role="sub"):
        docs = _phase_docs(camp_dir, sub_id)
        if not docs:
            continue                      # 부재는 P3 이 말한다 — 같은 사실을 두 술어로 세지 않는다
        for name, doc in docs:
            who = doc.get("authored_by")
            if not isinstance(who, str) or not who:
                problems.append(f"P5 {sub_id}/{name}: authored_by 가 없다 — 서브 진행표를 누가 적었는지 "
                                f"표시가 없으면 자기저작과 사후 재저작이 구분되지 않는다")
            elif who in mains:
                problems.append(f"P5 {sub_id}/{name}: authored_by={who!r} — 메인이 서브의 진행표를 "
                                f"적었다(상향 회수는 문서기반이고, 회수분은 서브 저작으로 남는다)")
        stamps = [d.get("ended_utc") for _, d in docs if isinstance(d.get("ended_utc"), str)]
        if len(stamps) >= 2 and len(set(stamps)) == 1:
            problems.append(f"P5 {sub_id}: phase {len(stamps)}건의 ended_utc 가 모두 {stamps[0]} 로 "
                            f"같다 — 같은 초에 통째로 저작된 진행표는 관측이 아니라 회고다")
    for brief in (brief_paths or []):
        try:
            doc = json.loads(Path(brief).read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"P5 brief 파손({brief}): {exc}")
            continue
        if not isinstance(doc, dict):
            continue
        if doc.get("self_role") != "sub":
            problems.append(f"P5 brief {brief}: self_role={doc.get('self_role')!r} — 회수된 브리핑은 "
                            f"서브가 자기 인스턴스에서 낸 것이어야 한다")
        if doc.get("authored_by") in mains:
            problems.append(f"P5 brief {brief}: authored_by={doc.get('authored_by')!r} 가 메인이다")
    return problems


def discover_briefs(camp_dir: Path) -> list:
    """회수된 서브 브리핑. 메인은 `fetch_sub_docs` 가 만든 미러 밖을 보지 않는다 —
    직접 스캔은 헌법 노드제어 ①(무단 스캔 금지) 위반이다."""
    mirror = REPO_ROOT / "sync_staging" / "sub_docs"
    if not mirror.is_dir():
        return []
    return sorted(mirror.glob("logs/*/campaign_brief.json"))


def acceptance_predicates(camp_dir: Path) -> list[str]:
    """P4·P5. 합격 판정(⑥)과 fetch 종료부가 부르고, **purge 게이트는 부르지 않는다**."""
    return predicate_p4(camp_dir) + predicate_p5(camp_dir, brief_paths=discover_briefs(camp_dir))


def validate_template() -> list[str]:
    """뼈대 자체의 완결성 — 사용자가 관리하는 유일한 부분이므로 모양이 무너지면 즉시 안다."""
    problems: list[str] = []
    required = [
        "campaign.schema.json", "campaign.yaml", "evidence_pointers.json", "residue.json",
        "cells/_cell/config.yaml", "cells/_cell/lockset.json", "cells/_cell/cell.status.json",
        "phases/_node/build.status.json", "phases/_node/serve.status.json",
        "phases/_node/bench.status.json", "phases/_node/publish.status.json",
        "relay/.gitkeep", "sweeps/.gitkeep",
    ]
    for rel in required:
        if not (TEMPLATE / rel).exists():
            problems.append(f"뼈대 결손: campaigns/_template/{rel}")
    # 뼈대는 빈칸을 **가지고 있어야** 한다 — 빈칸이 사라지면 사용자가 채울 자리가 없다는 뜻이다.
    tpl = TEMPLATE / "campaign.yaml"
    if tpl.is_file() and not find_fill_placeholders(tpl.read_text(encoding="utf-8")):
        problems.append("뼈대 campaign.yaml 에 <<FILL>> 이 하나도 없다 — 빈칸이 곧 계약이다")
    return problems


def _write_brief(path: Path, doc: dict) -> Path:
    path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return path


def _selftest() -> int:
    """음성대조 포함. 라이브 트리는 깨끗할 때 아무것도 증명하지 않는다 — 양성이 발화해야 한다."""
    import tempfile
    ok = True

    def ck(label: str, cond: bool) -> None:
        nonlocal ok
        print(("  PASS " if cond else "  FAIL ") + label)
        ok = ok and bool(cond)

    ck("뼈대 완결", not validate_template())
    ck("빈칸 검출", find_fill_placeholders(f"a: {FILL}\nb: 1\n") == [1])

    base = json.loads((TEMPLATE / "campaign.yaml").read_text(encoding="utf-8"))
    base.pop("_howto", None)
    good = dict(base, id="camp-x", plan_ref="docs/plan/p.md", declared_utc="2026-09-06T00:00:00Z",
                nodes=[{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"}],
                matrix={"versions": ["0.18.0"], "models": ["gpt-oss-20b"]},
                assignments={"main": [{"cell": "cell-a"}]},
                budgets={"smoke_budget_overhead_mib": 12265, "ready_max_seconds": 600},
                control_variables={"model": "gpt-oss-20b", "vllm_version": "0.18.0",
                                   "topology": "single", "target_gpu": "H100"},
                hint_targets=[{"arch": "gb10-main-sim-h100", "node_id": "main", "cells": []}])

    with tempfile.TemporaryDirectory() as tmp:
        camp = Path(tmp) / "camp-x"
        (camp / "cells" / "cell-a").mkdir(parents=True)
        (camp / "cells" / "cell-a" / "config.yaml").write_text("cell_id: cell-a\n", encoding="utf-8")
        cp = camp / "campaign.yaml"

        def write(doc: dict) -> list[str]:
            cp.write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
            return validate_campaign(cp)

        def _as(cells, node: str = "main", modes=None):
            modes = modes or {}
            return {node: [({"cell": c, "mode": modes[c]} if c in modes else {"cell": c})
                           for c in cells]}

        ck("정상 선언 통과", not write(good))
        ck("빈칸 잔존 차단", any(FILL in p for p in write(dict(good, plan_ref=FILL))))
        ck("예산 기본값 차단",
           any("smoke_budget_overhead_mib" in p
               for p in write(dict(good, budgets={"smoke_budget_overhead_mib": 0, "ready_max_seconds": 600}))))
        ck("통제변인 공란 차단",
           any("control_variables.model" in p
               for p in write(dict(good, control_variables=dict(good["control_variables"], model="")))))
        ck("옛 arch 문법 차단(노드 축 부재)",
           any("HINT_ARCH_NODE_AXIS_ABSENT" in p
               for p in write(dict(good, hint_targets=[{"arch": "gb10-sim-h100", "node_id": "main", "cells": []}]))))
        ck("미지의 노드 참조 차단",
           any("nodes[] 에 없다" in p
               for p in write(dict(good, hint_targets=[{"arch": "gb10-sub-sim-h100", "node_id": "ghost", "cells": []}]))))
        ck("예약 id 차단", any("예약 id" in p for p in write(dict(good, id="_bootstrap"))))
        ck("부재 셀 차단",
           any("입력이 없다" in p for p in write(dict(good, assignments=_as(["cell-missing"])))))
        # ★ 2026-09-06: order 를 cells/ 디렉터리 목록에서 파생하면 스캐폴드가 남긴 틀
        #   `_cell` 이 그대로 섞인다. 틀의 config.yaml 은 **실재하므로** 실재 검사만으로는
        #   통과했다(실제로 통과시켰다 — 이 시험이 그 재발을 막는다).
        (camp / "cells" / "_cell").mkdir(parents=True)
        (camp / "cells" / "_cell" / "config.yaml").write_text(
            f"target_model:\n  path: {FILL}\n", encoding="utf-8")
        ck("★틀 디렉터리가 배정에 들어가면 차단",
           any("틀은 실행 대상이 아니다" in p for p in write(dict(good, assignments=_as(["_cell"])))))
        # 셀 입력의 빈칸도 계약이다. `_cell` 은 이름으로 먼저 걸리므로 **다른 이름**으로 시험한다
        #   — 같은 픽스처가 두 규칙을 겸하면 어느 쪽이 발화했는지 모른다.
        (camp / "cells" / "cell-b").mkdir(parents=True)
        (camp / "cells" / "cell-b" / "config.yaml").write_text(
            f"target_model:\n  path: {FILL}\n", encoding="utf-8")
        ck("★셀 입력에 빈칸이 남으면 차단",
           any("config.yaml 에" in p and FILL in p for p in write(dict(good, assignments=_as(["cell-b"])))))
        (camp / "cells" / "cell-b" / "lockset.json").write_text(
            '{"id": "' + FILL + '"}\n', encoding="utf-8")
        (camp / "cells" / "cell-b" / "config.yaml").write_text("target_model:\n  path: /x\n",
                                                               encoding="utf-8")
        ck("★lockset 의 빈칸도 차단",
           any("lockset.json 에" in p for p in write(dict(good, assignments=_as(["cell-b"])))))
        # 음성대조: 빈칸을 채우면 같은 셀이 통과한다(과잉차단 아님)
        (camp / "cells" / "cell-b" / "lockset.json").write_text('{"id": "cell-b"}\n',
                                                                encoding="utf-8")
        ck("★음성대조: 빈칸을 채운 셀은 통과", not write(dict(good, assignments=_as(["cell-b"]))))
        ck("스키마 미지 필드 차단", any("campaign.yaml" in p for p in write(dict(good, surprise=1))))

        # ── 안내문 면제 (2026-09-08 · D10). 안내문이 자기 스캔에 걸리면 사람이 그것을 지운다.
        ck("★`_` 접두 최상위 안내문에 든 <<FILL>> 은 면제된다",
           not write(dict(good, _howto=[f"빈칸은 {FILL} 로 적는다"])))
        ck("★음성대조: 같은 문자열이 진짜 값 자리에 있으면 여전히 차단",
           any(FILL in p for p in write(dict(good, declared_utc=FILL))))
        ck("★깊은 자리의 `_` 키는 면제되지 않는다(빈칸이 거기 숨는다)",
           any(FILL in p for p in write(dict(good,
               topology_sections={"single": {"_note": FILL}, "multi": {}}))))

        # ── 배정 계층 (2026-09-08 · D11) ─────────────────────────────────────────────
        (camp / "cells" / "cell-c").mkdir(parents=True, exist_ok=True)
        (camp / "cells" / "cell-c" / "config.yaml").write_text("cell_id: cell-c\n", encoding="utf-8")
        two = {"main": [{"cell": "cell-a"}], "subx": [{"cell": "cell-c"}]}
        nodes2 = [{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"},
                  {"node_id": "subx", "role": "sub", "topology": "single", "hw": "gb10"}]
        ck("★노드별 배정이 통과한다(메인 1셀 ∥ 서브 1셀)",
           not write(dict(good, nodes=nodes2, assignments=two)))
        ck("★STAY 가 마지막이 아니면 차단",
           any("STAY" in p and "마지막" in p for p in write(dict(good,
               assignments=_as(["cell-a", "cell-b"], modes={"cell-a": "STAY"})))))
        ck("★음성대조: STAY 가 마지막이면 통과",
           not write(dict(good, assignments=_as(["cell-a", "cell-b"], modes={"cell-b": "STAY"}))))
        ck("mode 생략은 AUTO 다",
           cell_mode({"assignments": _as(["cell-a"])}, "main", "cell-a") == "AUTO"
           and cell_mode({"assignments": _as(["cell-a"], modes={"cell-a": "HITL"})},
                         "main", "cell-a") == "HITL")
        ck("★알 수 없는 mode 는 스키마가 차단",
           any("mode" in p for p in write(dict(good,
               assignments=_as(["cell-a"], modes={"cell-a": "LATER"})))))
        ck("★같은 셀의 이중 배정 차단(증거 태그가 갈린다)",
           any("이중 배정" in p for p in write(dict(good, nodes=nodes2,
               assignments={"main": [{"cell": "cell-a"}], "subx": [{"cell": "cell-a"}]}))))
        ck("★선언되지 않은 노드에 배정하면 차단",
           any("nodes[] 에 없다" in p for p in write(dict(good, assignments=_as(["cell-a"], node="ghost")))))
        ck("★배정이 비면 차단", any("assignments 가 비었다" in p for p in write(dict(good, assignments={}))))
        ck("★hint_targets.cells 가 배정 밖이면 차단(배정 SSOT 는 assignments)",
           any("assignments" in p and "파생" in p for p in write(dict(good,
               hint_targets=[{"arch": "gb10-main-sim-h100", "node_id": "main",
                              "cells": ["cell-zzz"]}]))))
        write(good)

        # phase proof 음성대조
        (camp / "phases" / "main").mkdir(parents=True)
        write(good)
        st = camp / "phases" / "main" / "serve.status.json"
        st.write_text(json.dumps({"schema_version": 1, "node_id": "main", "phase": "serve",
                                  "cell_id": "cell-a", "state": "done",
                                  "proof": {"predicate": "health200", "ok": True, "source": ""}}), encoding="utf-8")
        ck("출처 없는 proof 차단", any("source 가 비었다" in p for p in validate_instance(camp)))
        st.write_text(json.dumps({"schema_version": 1, "node_id": "main", "phase": "serve",
                                  "cell_id": "cell-a", "state": "done",
                                  "proof": {"predicate": "health200", "ok": True,
                                            "source": "docs/testlog/t.md"}}), encoding="utf-8")
        ck("출처 있는 proof 통과", not validate_instance(camp))

        # ── P1~P3 (2026-09-07 · plan_26090715 §4.4). 완주를 표현할 술어가 없어서 캠페인 ⑦ 이
        #    전부-pending 인 채로 PASS 했다. 셋 다 인스턴스 안의 데이터만으로 계산된다.
        (camp / "sweeps").mkdir(parents=True, exist_ok=True)
        (camp / "sweeps" / "s1.json").write_text(json.dumps({"cells": [
            {"cell_key": "cell-a", "cell_outcome": "measured"},
            {"cell_key": "cell-z", "cell_outcome": "serve_failed"}]}), encoding="utf-8")
        (camp / "cells" / "cell-a").mkdir(parents=True, exist_ok=True)
        (camp / "cells" / "cell-a" / "cell.status.json").write_text(
            json.dumps({"schema_version": 1, "cell_id": "cell-a", "cell_outcome": "pending"}),
            encoding="utf-8")
        p1 = predicate_p1(camp)
        ck("★P1 sweep='measured' vs cell.status='pending' 불일치 검출",
           any("cell-a" in x and "불일치" in x for x in p1))
        ck("★P1 sweep 은 돌았다는데 cell.status.json 자체가 없으면 검출",
           any("cell-z" in x and "없다" in x for x in p1))
        (camp / "cells" / "cell-a" / "cell.status.json").write_text(
            json.dumps({"schema_version": 1, "cell_id": "cell-a", "cell_outcome": "measured"}),
            encoding="utf-8")
        (camp / "cells" / "cell-z").mkdir(parents=True, exist_ok=True)
        (camp / "cells" / "cell-z" / "cell.status.json").write_text(
            json.dumps({"schema_version": 1, "cell_id": "cell-z", "cell_outcome": "serve_failed"}),
            encoding="utf-8")
        ck("일치하면 P1 통과", not predicate_p1(camp))
        (camp / "sweeps" / "s1.json").unlink()
        ck("sweep 이 없으면 P1 은 아무 말도 하지 않는다(부재 ≠ 불일치)", not predicate_p1(camp))

        (camp / "evidence_pointers.json").write_text(json.dumps({"pointers": [
            {"kind": "certificate", "path": "CLAUDE.md", "node_id": "main"},
            {"kind": "bench_report", "path": "README.md", "node_id": "subx"}]}), encoding="utf-8")
        p2 = predicate_p2(camp)
        ck("★P2 벤치 증거가 왔는데 bench phase 가 없으면 검출",
           any("subx" in x and "없다" in x for x in p2))
        ck("★P2 벤치 증거가 왔는데 bench phase 가 pending 이면 검출(main)",
           any("main" in x for x in p2))
        (camp / "phases" / "main" / "bench.status.json").write_text(json.dumps(
            {"schema_version": 1, "node_id": "main", "phase": "bench", "state": "done",
             "proof": {"predicate": "리포트 실재", "ok": True, "source": "docs/benchmark/r.md"}}),
            encoding="utf-8")
        ck("bench 가 done+ok 면 그 노드는 P2 를 통과한다",
           not any(x.startswith("P2 main") for x in predicate_p2(camp)))
        (camp / "evidence_pointers.json").unlink()

        write(dict(good, nodes=[{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"},
                                {"node_id": "ghost", "role": "sub", "topology": "single", "hw": "gb10"}]))
        p3 = predicate_p3(camp)
        ck("★P3 선언된 노드에 phases/ 가 없으면 검출",
           any("ghost" in x for x in p3) and not any("P3 main" in x for x in p3))
        (camp / "phases" / "ghost").mkdir(parents=True, exist_ok=True)
        ck("phases/ 가 생기면 P3 통과", not predicate_p3(camp))

        # ── 증거 빈칸: 검증기와 purge 게이트가 같은 것을 보는가 (2026-09-08 §A F1) ─────────
        write(good)
        (camp / "evidence_pointers.json").write_text(json.dumps({
            "schema_version": 1, "campaign_id": FILL,
            "_pointer_shape": {"kind": f"예시 {FILL}"},
            "pointers": [{"kind": FILL, "path": FILL, "cell_id": None, "node_id": None}]}),
            encoding="utf-8")
        ck("★증거 포인터의 <<FILL>> 스텁을 검증기가 잡는다(종전엔 purge 게이트만 잡았다)",
           any("evidence_pointers.json" in x and FILL in x for x in validate_instance(camp)))
        (camp / "evidence_pointers.json").write_text(json.dumps({
            "schema_version": 1, "campaign_id": "camp-x",
            "_pointer_shape": {"kind": f"예시 {FILL}"}, "pointers": []}), encoding="utf-8")
        ck("★음성대조: 빈 목록 + 안내문만 남으면 통과(안내문은 면제)",
           not any("evidence_pointers.json" in x for x in validate_instance(camp)))
        (camp / "evidence_pointers.json").unlink()

        # ── P2 강화 · P4 · P5 (2026-09-08 · plan_26090813 §4.7) ──────────────────────────
        c2 = Path(tmp) / "camp-2"
        (c2 / "cells" / "cell-a").mkdir(parents=True)
        (c2 / "cells" / "cell-a" / "config.yaml").write_text("cell_id: cell-a\n", encoding="utf-8")
        (c2 / "cells" / "cell-c").mkdir(parents=True)
        (c2 / "cells" / "cell-c" / "config.yaml").write_text("cell_id: cell-c\n", encoding="utf-8")
        (c2 / "relay").mkdir(parents=True)

        def decl2(nodes, assigns):
            (c2 / "campaign.yaml").write_text(json.dumps(dict(
                good, id="camp-2", nodes=nodes, assignments=assigns, hint_targets=[]),
                ensure_ascii=False), encoding="utf-8")

        two_nodes = [{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"},
                     {"node_id": "sub", "role": "sub", "topology": "single", "hw": "gb10"}]
        decl2(two_nodes, {"main": [{"cell": "cell-a"}], "sub": [{"cell": "cell-c"}]})
        (c2 / "evidence_pointers.json").write_text(json.dumps({"pointers": [
            {"kind": "certificate", "path": "CLAUDE.md", "cell_id": "cell-a", "node_id": None},
            {"kind": "bench_report", "path": "README.md", "cell_id": "cell-c", "node_id": None}]}),
            encoding="utf-8")
        p2 = predicate_p2(c2)
        ck("★P2 다노드에서 노드 태그 없는 벤치 증거를 검출(종전엔 침묵)",
           sum(1 for x in p2 if "노드 태그 없는" in x) == 2)
        ck("★P2 전부 무태그면 '공허 통과 금지' 를 명시한다",
           any("공허 통과" in x for x in p2))
        decl2([two_nodes[0]], {"main": [{"cell": "cell-a"}]})
        ck("★단일노드는 태그 부재가 결함이 아니다(귀속이 자명하다 — 과잉차단 ✗)",
           not any("노드 태그 없는" in x or "공허 통과" in x for x in predicate_p2(c2)))
        ck("단일노드에서는 그 하나의 노드에 귀속되어 bench phase 를 묻는다",
           any(x.startswith("P2 main") for x in predicate_p2(c2)))

        decl2(two_nodes, {"main": [{"cell": "cell-a"}], "sub": [{"cell": "cell-c"}]})
        (c2 / "evidence_pointers.json").unlink()
        ck("★P4 는 메인 시각이 없으면 그 사실을 말한다",
           any("시각이 하나도 없다" in x for x in predicate_p4(c2)))
        (c2 / "phases" / "main").mkdir(parents=True)
        (c2 / "phases" / "main" / "build.status.json").write_text(json.dumps({
            "schema_version": 1, "node_id": "main", "phase": "build", "state": "done",
            "first_started_utc": "2026-09-08T10:00:00Z", "started_utc": "2026-09-08T20:00:00Z",
            "proof": {"predicate": "이미지 실재", "ok": True, "source": "docker inspect"}}),
            encoding="utf-8")
        ck("★P4 서브 빌드 원장이 없으면 검출(선발급의 관측 자리가 비었다)",
           any("빌드 context 원장이 없다" in x for x in predicate_p4(c2)))

        def ledger(started):
            (c2 / "relay" / "camp2-sub-build.json").write_text(json.dumps({
                "schema_version": 1, "context_id": "camp2-sub-build", "campaign_id": "camp-2",
                "campaign_node": "sub", "campaign_context_kind": "build",
                "attempts": [{"attempt": 1, "started_utc": started}]}), encoding="utf-8")
        ledger("2026-09-08T11:30:00Z")
        ck("★P4 서브 지시서가 메인 첫 착수보다 늦으면 검출(= '순차로 돌았다')",
           any("보다 늦다" in x for x in predicate_p4(c2)))
        ledger("2026-09-08T09:50:00Z")
        ck("★P4 음성대조: 서브 지시서가 먼저 나가면 통과", not predicate_p4(c2))
        # started_utc 만 있고 first_started_utc 가 없으면 마지막 셀 시각을 읽어 **늦게** 본다.
        (c2 / "phases" / "main" / "build.status.json").write_text(json.dumps({
            "schema_version": 1, "node_id": "main", "phase": "build", "state": "done",
            "started_utc": "2026-09-08T09:00:00Z",
            "proof": {"predicate": "이미지 실재", "ok": True, "source": "docker inspect"}}),
            encoding="utf-8")
        ck("★P4 는 first_started_utc 가 없으면 started_utc 로 폴백한다(부재 ≠ 통과)",
           any("보다 늦다" in x for x in predicate_p4(c2)))

        (c2 / "phases" / "sub").mkdir(parents=True)
        for _ph in ("build", "serve"):
            (c2 / "phases" / "sub" / f"{_ph}.status.json").write_text(json.dumps({
                "schema_version": 1, "node_id": "sub", "phase": _ph, "state": "done",
                "ended_utc": "2026-09-08T22:34:11Z",
                "proof": {"predicate": "x", "ok": True, "source": "y"}}), encoding="utf-8")
        p5 = predicate_p5(c2)
        ck("★P5 authored_by 가 없으면 검출(자기저작과 사후 재저작이 구분 안 된다)",
           sum(1 for x in p5 if "authored_by 가 없다" in x) == 2)
        ck("★P5 동일초 통째 저작을 검출(camp-26090721 의 22:34:11Z 재발 방지)",
           any("같다" in x and "회고" in x for x in p5))
        for _i, _ph in enumerate(("build", "serve")):
            (c2 / "phases" / "sub" / f"{_ph}.status.json").write_text(json.dumps({
                "schema_version": 1, "node_id": "sub", "phase": _ph, "state": "done",
                "authored_by": "sub", "ended_utc": f"2026-09-08T2{_i}:00:00Z",
                "proof": {"predicate": "x", "ok": True, "source": "y"}}), encoding="utf-8")
        ck("★P5 음성대조: 서브가 서로 다른 시각에 적으면 통과", not predicate_p5(c2))
        (c2 / "phases" / "sub" / "build.status.json").write_text(json.dumps({
            "schema_version": 1, "node_id": "sub", "phase": "build", "state": "done",
            "authored_by": "main", "ended_utc": "2026-09-08T20:00:00Z",
            "proof": {"predicate": "x", "ok": True, "source": "y"}}), encoding="utf-8")
        ck("★P5 메인이 서브 진행표를 적으면 검출",
           any("메인이 서브의 진행표를" in x for x in predicate_p5(c2)))
        ck("★P5 회수 brief 의 self_role 이 sub 가 아니면 검출",
           any("self_role" in x for x in predicate_p5(
               c2, brief_paths=[_write_brief(Path(tmp) / "brief.json", {"self_role": "main"})])))
    print("[campaign_template_validator] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--campaign", help="검증할 campaign.yaml 경로")
    ap.add_argument("--instance", help="검증할 campaigns/<id>/ 디렉터리")
    ap.add_argument("--template", action="store_true", help="뼈대 자체를 검증")
    ap.add_argument("--acceptance", metavar="INSTANCE",
                    help="합격 술어 P4(동시 착수 타임라인)·P5(서브 자기저작)만 판정한다 — "
                         "purge 게이트와 분리된 자리다(진행을 막지 않는다)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.selftest:
            return _selftest()
        problems: list[str] = []
        if a.template or not (a.campaign or a.instance or a.acceptance):
            problems += validate_template()
        if a.campaign:
            problems += validate_campaign(Path(a.campaign))
        if a.instance:
            problems += validate_instance(Path(a.instance))
        if a.acceptance:
            problems += acceptance_predicates(Path(a.acceptance))
    except CampaignContractFailure as exc:
        print(f"[campaign_template_validator] FAIL {exc}", file=sys.stderr)
        return 1
    if problems:
        print(f"[campaign_template_validator] FAIL ({len(problems)})", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1
    print("[campaign_template_validator] PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
