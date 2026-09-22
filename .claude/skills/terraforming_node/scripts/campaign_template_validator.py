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
# 셀 출처(2026-09-14 · plan_26091407 §4.0 · 사용자 결정 Q1·Q8). **어휘의 단일 소유자는 이 파일이다** —
#   측정 진입 precheck(`campaign_init --lockset-precheck` ← `broad_search.sh cell`)와 합격 술어 P6 가
#   같은 목록을 읽는다. 두 자리에 적으면 한쪽이 조용히 늦는다(_SWEEP_TO_CELL 선례).
#   손작성은 금지가 아니라 표시 대상이다: `hand-authored` 는 통과하고, fail-closed 는 **표시 부재**뿐이다.
LOCKSET_PROVENANCE = ("explorer-phase2", "hand-authored")
# 예외 노브의 출처 어휘. 값이 없으면(null) '아직 정하지 않았다'이고, 목록 밖 값만 P6 가 기재한다(차단 ✗).
LOCKSET_KNOB_SOURCES = {
    "batch_source": ("declared-requirement", "kv-fit-measured", "hand-lever"),
    "gmu_source": ("target_gmu", "hand"),
    "kv_source": ("measured-clamp", "hand"),
}
# hint 발행 사전승인(2026-09-21 · plan_26092119 O6 · SPEC X3). **승인 모양의 단일 소유자는 이 파일이다** —
#   writer(`campaign_init --hint-approve`)는 쓰기 전에 여기 판정을 부르고, 읽는 쪽(hint 발행기 `continue`)은
#   `hint_approval_for` 를 부른다. 세 자리에 모양을 적으면 한쪽이 조용히 늦는다(LOCKSET_PROVENANCE 선례).
#   스키마 enum(`definitions.hint_approval.properties.source.enum`)은 안내 사본이고 뼈대 검증이 교차대조한다.
HINT_APPROVAL_FIELDS = ("approved_by", "approved_utc", "source")
HINT_APPROVAL_SOURCES = ("declaration-popup", "publish-popup")
# 선언 확인 팝업이 O6 의 기본 승인 자리다(진행을 막는 세 자리 중 하나 · workflow.md 2026-09-08 D17).
HINT_APPROVAL_DEFAULT_SOURCE = "declaration-popup"
# 폐기된 hint_target 키. `arch` = 2026-09-21 D8(이름은 도구가 전량 파생) — 손으로 적은 arch 는 파생 가능한
#   값의 손사본이고, 그 검사 때문에 이 검증기가 옛 발행기 모듈을 파일 적재하던 **양방향 결합**이 있었다.
#   그 적재기는 모듈이 없으면 None 을 돌려 검사를 조용히 건너뛰었다(fail-open · 코드맵 H3) — 키를 폐기하면서
#   적재기도 함께 걷어냈다. 이 목록은 남은 키가 **왜** 거부되는지를 말하기 위한 닫힌 목록(tripwire)이다.
HINT_TARGET_RETIRED_KEYS = {"arch": "태그 이름은 발행 도구가 전량 파생한다(2026-09-21 D8) — "
                                    "손으로 적은 arch 는 '파생 가능한데 손으로 적은 값'이다"}
# 주입 시각 모양. 발행 페이로드가 이 값을 그대로 옮기고 발행기는 같은 모양만 받는다 — 모양이 다른 승인은
#   봉인 직전에야 터진다. 저장소 관행(블랙박스 ISO_RE 등)과 같은 국소 상수다.
APPROVAL_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")


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


# ── hint 발행 사전승인 (2026-09-21 · plan_26092119 O6 · SPEC X3) ─────────────────────────────
# 왜: O6 는 "셀마다 팝업" 대신 **선언 확인 팝업에서 발행 대상 셀 목록을 사전승인**하기로 했다(AUTO
#   캠페인이 셀마다 멈추지 않게 · 2026-09-08 "진행을 막는 자리는 셋뿐" 정합). 그런데 그 팝업은 산문에만
#   있고 **기계 기록이 어디에도 없었다**(코드맵 G3) — 기록 없는 승인은 다음 세션에서 사라진다. 그래서
#   승인을 hint_targets 항목으로 적고, 모양의 판정을 여기 한 곳에 둔다. 무인 자동 태깅 ✗ 는 그대로다:
#   사람이 명시 셀 목록을 승인했다는 사실을 전사한 것이지 기계가 고른 것이 아니다.

def retired_hint_key_reasons(doc: dict) -> list[str]:
    """폐기된 hint_target 키가 남은 자리와 **그 이유**. 스키마(additionalProperties:false)도 막지만 스키마
    메시지는 '모르는 키' 라고만 말한다 — 이유를 모르면 사람은 키 이름만 바꿔 다시 넣는다."""
    out: list[str] = []
    targets = doc.get("hint_targets")
    for i, target in enumerate(targets if isinstance(targets, list) else []):
        if not isinstance(target, dict):
            continue
        for key, why in HINT_TARGET_RETIRED_KEYS.items():
            if key in target:
                out.append(f"hint_targets[{i}].{key} 는 폐기된 키다 — {why}. 키를 지우고 승인은 "
                           f"`campaign_init.py --hint-approve` 로 적는다")
    return out


def hint_approval_reason(approval) -> str | None:
    """승인 한 건의 결함 사유(None = 완결). 세 필드 모두 비-빈 · 빈칸 없음 · 시각은 주입 모양 · 출처는 목록 안.

    반쪽 승인(누가만 있고 언제가 없는 것)은 승인이 아니다 — `--revise` 의 4필드 규율과 같은 이유다
    ("넷 중 하나라도 없으면 그것은 개정이 아니라 드리프트다")."""
    if not isinstance(approval, dict):
        return "approval 이 없다 — 사람 승인의 전사가 없는 항목은 사전승인이 아니다"
    for field in HINT_APPROVAL_FIELDS:
        val = approval.get(field)
        if not isinstance(val, str) or not val.strip():
            return f"approval.{field} 가 비었다 — 반쪽 승인은 승인이 아니다"
        if FILL in val:
            return f"approval.{field} 에 {FILL} 이 남아 있다 — 모르는 값을 그럴듯하게 채우지 않는다"
    if not APPROVAL_UTC_RE.match(approval["approved_utc"]):
        return (f"approval.approved_utc {approval['approved_utc']!r} 가 주입 시각 모양(YYYY-MM-DDTHH:MM:SSZ)이 "
                f"아니다 — 발행 페이로드는 이 모양만 옮긴다")
    if approval["source"] not in HINT_APPROVAL_SOURCES:
        return f"approval.source {approval['source']!r} 가 목록 밖이다(허용 {list(HINT_APPROVAL_SOURCES)})"
    return None


def hint_target_reasons(doc: dict) -> list[str]:
    """hint_targets 전체의 결함(빈 리스트 = 통과). validate_campaign 과 writer(`--hint-approve`)가 같은 판정을
    부른다 — writer 는 검증기가 거부할 선언을 만들지 않는다."""
    problems: list[str] = list(retired_hint_key_reasons(doc))
    targets = doc.get("hint_targets")
    if targets is None:
        return problems
    if not isinstance(targets, list):
        return problems + ["hint_targets 가 목록이 아니다"]
    if targets and doc.get("self_role") == "sub":
        # 발행은 메인 소관이다 — 파생 선언은 hint_targets 를 비워 보내고(emit_slice), 서브는 발행기를 켜지
        # 않는다(orchestration.topology.md). 서브 인스턴스의 승인 기록은 읽는 쪽이 없는 거짓 약속이다.
        problems.append("self_role=sub 인스턴스에 hint_targets 가 있다 — 발행 승인은 메인 선언에만 적는다")
    node_ids = {n.get("node_id") for n in (doc.get("nodes") or []) if isinstance(n, dict)}
    approved_at: dict = {}
    for i, target in enumerate(targets):
        if not isinstance(target, dict):
            problems.append(f"hint_targets[{i}] 가 객체가 아니다")
            continue
        nid = target.get("node_id")
        if nid not in node_ids:
            problems.append(f"hint_targets[{i}].node_id {nid!r} 가 nodes[] 에 없다")
            continue
        cells = target.get("cells")
        if not isinstance(cells, list) or not cells:
            # ★ 2026-09-21: 종전엔 '비우면 그 노드의 전 셀' 이었다. 사전승인에서 그 해석은 빈 목록을
            #   백지 승인으로 만든다 — 사람이 보지 않은 셀까지 승인된 것으로 읽힌다(코드맵 §3.2).
            problems.append(f"hint_targets[{i}].cells 가 비었다 — 승인할 셀을 명시 열거한다"
                            f"(빈 목록 = 전 셀 해석은 폐기 · 백지 승인 ✗)")
            cells = []
        # 배정 SSOT 는 assignments 이고 hint_targets 는 **파생**이다. 두 자리가 갈라지면 태그가
        # 자기가 요약하지 않은 셀을 주장한다 — 이름이 곧 증거 연결이라 그 주장은 게이트를 지난다.
        assigned_here = set(assigned_cells(doc, nid))
        for c in cells:
            if not isinstance(c, str) or not c.strip() or FILL in c:
                problems.append(f"hint_targets[{i}].cells 에 빈 셀 이름 {c!r} 이 있다")
                continue
            if c not in assigned_here:
                problems.append(f"hint_targets[{i}].cells 의 {c!r} 이 assignments[{nid!r}] 에 없다 "
                                f"— 배정 SSOT 는 assignments 이고 hint_targets 는 파생이다")
            if c in approved_at:
                problems.append(f"hint_targets[{i}].cells 의 {c!r} 은 이미 hint_targets[{approved_at[c]}] 가 "
                                f"승인했다 — 한 셀의 승인은 하나다(어느 전사가 그 셀의 승인인지 갈리지 않는다)")
            else:
                approved_at[c] = i
        why = hint_approval_reason(target.get("approval"))
        if why is not None:
            problems.append(f"hint_targets[{i}] {why}")
    return problems


def hint_approval_for(doc: dict, cell: str, node: str | None = None) -> dict | None:
    """이 셀의 발행 사전승인(없으면 None). **읽는 쪽의 판정 원천**이다 — hint 발행기 `continue` 는 승인을
    직접 파싱하지 않고 이 함수를 부른다(모양 판정이 두 벌이 되지 않게).

    fail-closed: hint_targets 에 결함이 하나라도 있으면 어떤 승인도 읽지 않는다 — 모양이 무너진 기록에서
    어느 줄이 진짜 승인인지 가를 수 없다. 사유는 `hint_target_reasons(doc)` 가 말한다.
    `node` 를 주면 그 선언 노드의 항목만 본다(측정 노드 축 → 선언 노드 해소는 호출부 몫 ·
    `resolve_measurement_node`)."""
    if hint_target_reasons(doc):
        return None
    for i, target in enumerate(doc.get("hint_targets") or []):
        if node is not None and target.get("node_id") != node:
            continue
        if cell in (target.get("cells") or []):
            return {"index": i, "node_id": target["node_id"], "cells": list(target["cells"]),
                    "approval": {k: target["approval"][k] for k in HINT_APPROVAL_FIELDS}}
    return None


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
    # 폐기 키는 스키마도 막지만 스키마는 '모르는 키' 라고만 말한다 — 조기 반환 전에 이유를 함께 싣는다.
    problems += retired_hint_key_reasons(doc)
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

    # hint 발행 사전승인(2026-09-21 · O6): 노드 실재 · 명시 셀 ⊆ 배정 · 승인 전사 완결 · 한 셀 한 승인.
    #   ★ 이 자리는 종전에 옛 발행기 모듈을 파일 적재해 arch 문법을 물었고, 모듈이 없으면 검사를 조용히
    #   건너뛰었다(fail-open · 코드맵 H3). arch 가 폐기되면서(D8) 이 검증기는 발행기에서 아무것도 읽지
    #   않는다 — 남은 간선은 발행기 → 이 검증기 한 방향뿐이다(plan §4.9 순환 제거).
    problems += hint_target_reasons(doc)

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


# ─────────────────────────────────────────────────────────────────────────────
# 측정노드 어휘 → 진행노드 어휘 (2026-09-11 신설 · plan_26091108 R0)
#
# 왜: 두 어휘가 갈라져 있었고 P2 가 그 둘을 **문자열 동일성**으로 이었다.
#   · 증거 쪽 — `sweep_bench.sh` 는 multi 토폴로지에서 `measured_node="cluster"` 를 파생한다
#     ("쌍이 하나의 측정 정체성"). 그 값이 인증서에 실려 `--evidence-add --node cluster` 로 온다.
#   · 진행 쪽 — `phases/<node>/` 의 <node> 는 `campaign.yaml.nodes[]` 의 선언 노드(main·sub)다.
#   따라서 **multi 캠페인은 벤치 증거가 도착하는 순간 P2 가 반드시 실패한다** — phases/cluster/ 는
#   어떤 실행자도 만들지 않기 때문이다. purge 게이트가 구조적으로 열릴 수 없었다는 뜻이고,
#   실제로 camp-26090918(multi · 인증서 2건)에서 그 상태가 관측됐다.
#
# 처방: 파생 축을 **선언 노드 집합으로 해소**한다. `cluster` 는 "이 캠페인의 선언 노드들이 함께
#   낸 하나의 측정" 이므로, 그 측정을 구동한 노드 **하나**의 bench 진행표가 그것을 반영하면 족하다
#   (multi 에서 GuideLLM 은 API 서버가 뜬 노드에만 붙는다 — `run_bench.sh --network host`).
#   전 노드를 요구하면 Ray 워커(sub)의 없는 bench 진행표를 요구하게 되어 과잉차단이다.
#
# 이 목록은 **거울이다** — 정본은 producer(`sweep_bench.sh`)의 리터럴이고, 여기 사본이 갈라지면
#   P2 가 조용히 눈이 먼다. 그래서 셀프테스트가 producer 파일을 열어 리터럴 실재를 교차검증한다
#   (workflow.md §결정론 규율: "단일 소유가 불가능하면 교차검증이 차선이다").
DERIVED_MEASUREMENT_NODES = ("cluster",)

# 교차검증 앵커 — producer 의 코드 토큰(주석 ✗ · 리팩터를 따라간다).
_DERIVED_NODE_PRODUCER = (
    ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh",
    'measured_node, measured_node_source = "cluster"',
)


def resolve_measurement_node(tag: str, declared: list[str]) -> tuple[list[str], str]:
    """측정 증거의 node_id 를 진행표 노드 후보로 해소한다.

    반환 `(candidates, kind)`:
      · `([tag], "declared")`   — 선언 노드 그 자체. 그 노드의 bench 진행표를 묻는다.
      · `(declared, "derived")` — 파생 측정축(`cluster`). 후보 **중 하나**가 반영하면 통과.
      · `([], "unknown")`       — 어느 어휘에도 없다. 오타를 조용히 통과시키지 않는다.
    """
    if tag in declared:
        return [tag], "declared"
    if tag in DERIVED_MEASUREMENT_NODES:
        return list(declared), "derived"
    return [], "unknown"


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
        # 2026-09-11(R0): 증거의 노드 태그는 **측정 어휘**이고 phases/ 는 **진행 어휘**다.
        #   둘을 문자열 동일성으로 이으면 multi 의 파생축(`cluster`)에서 항상 실패한다.
        candidates, kind = resolve_measurement_node(node, nodes)
        if kind == "unknown":
            problems.append(
                f"P2 {node}: 벤치 증거 {len(paths)}건의 노드 태그가 선언 노드({', '.join(nodes) or '없음'})"
                f"에도 파생 측정축({', '.join(DERIVED_MEASUREMENT_NODES)})에도 없다 — "
                f"어느 노드가 잰 것인지 해소할 수 없다(오타 또는 미선언 노드)")
            continue
        seen: list[str] = []
        reflected = False
        for cand in candidates:
            status_path = camp_dir / "phases" / cand / "bench.status.json"
            if not status_path.is_file():
                continue
            try:
                st = json.loads(status_path.read_text(encoding="utf-8"))
            except (OSError, ValueError) as exc:
                problems.append(f"P2 {cand}: bench.status.json 파손 — {exc}")
                seen.append(cand)
                continue
            seen.append("%s(state=%r proof.ok=%r)"
                        % (cand, st.get("state"), (st.get("proof") or {}).get("ok")))
            if st.get("state") == "done" and (st.get("proof") or {}).get("ok") is True:
                reflected = True
                break
        if reflected:
            continue
        if not seen:
            where = (f"phases/{candidates[0]}/bench.status.json" if kind == "declared"
                     else "선언 노드 어디에도 bench.status.json")
            problems.append(f"P2 {node}: 벤치 증거 {len(paths)}건이 있는데 {where} 이 없다 "
                            f"— 진행표가 증거보다 낡았다")
            continue
        problems.append(f"P2 {node}: 벤치 증거가 도착했는데 bench phase 가 그것을 반영하지 않는다 "
                        f"— {'; '.join(seen)} (진행표가 증거보다 낡았다)")
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
    """Return only this campaign's briefs from the standing docs mirror."""
    mirror = REPO_ROOT / "sync_staging" / "sub_docs"
    if not mirror.is_dir():
        return []
    found = []
    for path in sorted(mirror.glob("logs/*/campaign_brief.json")):
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, ValueError):
            continue                    # unrelated historical mirror residue
        if isinstance(doc, dict) and doc.get("campaign_id") == camp_dir.name:
            found.append(path)
    return found


# ─────────────────────────────────────────────────────────────────────────────
# P6 — 셀 출처 표시 (2026-09-14 신설 · plan_26091407 §4.0 · 사용자 결정 Q1·Q8)
#
# 왜: 활성 캠페인 셀 11/11 에 lockset.json 이 없었고 서빙 yaml 은 explorer Phase-2 없이 손으로
#   적혔다(승자 셀 yaml 의 gmu 0.80 은 config target_gmu 0.85 와 달랐다). 그런데 이 검증기는 lockset
#   부재를 `continue` 로 넘겼다 — "측정 산물" 이라 적힌 값이 사람이 적은 값이었다는 사실을 가리는
#   자리가 0 이었다(F1).
# ★ 왜 acceptance 인가(purge 선행조건 ✗): 표시가 빠진 셀이 **다음 캠페인을 영원히 막는** 재발 패턴을
#   피한다(P4·P5 와 같은 이유 · plan §9 R3). 셀 출처로 진행을 막는 자리는 측정 진입 precheck 하나이고
#   (`broad_search.sh cell` → exit 2), 여기는 캠페인이 설계대로 표시를 남겼는지 **관측**한다.

def lockset_provenance_reason(path: Path) -> str | None:
    """lockset 출처 표시가 성립하지 않는 **사유**(None = 성립). precheck 와 P6 가 공유한다.

    fail-closed 대상은 표시 **부재·무효**뿐이다 — `hand-authored` 는 사유가 아니다(Q1: 손작성 허용).
    `<<FILL>>` 은 목록 밖 값으로 거부된다: 뼈대를 복사만 하고 채우지 않은 lockset 은 출처를 말하지
    않은 것이다.
    """
    if not path.is_file():
        return (f"lockset 부재: {_rel(path)} — 셀 출처(provenance)를 말할 자리가 없다"
                f"(셀 materialize 는 explorer 소관 · 손작성이면 provenance=hand-authored 로 표시)")
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return f"lockset 파손({_rel(path)}): {exc}"
    if not isinstance(doc, dict):
        return f"lockset 최상위가 객체가 아니다: {_rel(path)}"
    prov = doc.get("provenance")
    if prov is None:
        return (f"lockset provenance 부재: {_rel(path)} — 값 {LOCKSET_PROVENANCE} 중 하나로 출처를 "
                f"표시하라(손작성은 금지가 아니라 표시 대상이다)")
    if prov not in LOCKSET_PROVENANCE:
        return (f"lockset provenance 무효: {prov!r} ({_rel(path)}) — 허용 {LOCKSET_PROVENANCE}")
    return None


def lockset_knob_source_reasons(path: Path) -> list[str]:
    """예외 노브 `*_source` 의 목록 밖 값. null 은 '아직 정하지 않았다'라 사유가 아니다."""
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []                          # 파손은 provenance 사유가 이미 말한다(같은 사실을 두 번 세지 않는다)
    if not isinstance(doc, dict):
        return []
    out: list[str] = []
    for key, allowed in LOCKSET_KNOB_SOURCES.items():
        val = doc.get(key)
        if val is not None and val not in allowed:
            out.append(f"{key}={val!r} 가 목록 밖이다(허용 {allowed})")
    return out


def lockset_observable(decl: dict, node: str | None) -> bool:
    """이 인스턴스가 셀 lockset 을 **관측할 수 있는가**. P6 와 `--cell-set` writer 기재가 공유한다.

    메인 인스턴스에서 role=sub 노드에 배정된 셀은 서브가 자기 인스턴스에서 lockset 을 저작하고
    (측정 스킬이 tool_plane 으로 열린 서브의 precheck 가 진입에서 집행한다 · node_role_contract — tool_plane 이 비는
    서브에는 그 precheck 의 집행자가 없고, 서브 배정은 토폴로지 오케스트레이션 전략이 정한다 · 이 함수는 그것을 검사하지
    않는다), 메인은 그 인스턴스를 직접 읽지 않는다(헌법 노드제어 ① ·
    상향 회수는 문서기반 · 브리핑은 lockset 출처를 싣지 않는다). 그 자리의 "없음" 은 부재가 아니라
    **관측 불가**다 — 두 판정기가 이 구분을 따로 적으면 한쪽은 건너뛰고 다른 쪽은 "부재" 를 기재하는
    모순이 난다(2026-09-14 리뷰 실증). 파생 선언으로 열린 서브 인스턴스(self_role=sub)는 자기 배정
    전수를 관측한다.
    """
    if decl.get("self_role") == "sub":
        return True
    roles = {n.get("node_id"): n.get("role") for n in (decl.get("nodes") or []) if isinstance(n, dict)}
    return roles.get(node) != "sub"


def p6_observations(camp_dir: Path) -> "tuple[list[str], list[str]]":
    """P6 판정 `(problems, notes)`. problems 만 합격 여부를 가르고, notes 는 **이름으로 남기는 관측**이다.

    P6 의 합격 정의는 plan_26091407 §4.0 그대로 "관측 가능한 배정 셀 전수에 provenance 표시가 있다"
    이다. 거기서 벗어나는 두 사실은 적색이 아니라 notes 로 간다:
      · 관측 대상 밖 셀(메인 인스턴스의 서브 배정 셀) — 조용히 건너뛰면 "P6 초록" 이 그 셀들에 대해
        공허 통과가 된다. 건너뛴 셀을 이름으로 남긴다(writer 의 pending_knobs 와 같은 원칙).
      · 노브 `*_source` 의 목록 밖 값 — 합격 정의에 없는 조건으로 적색을 내면 E2E 술어 ⑥(P6 초록)의
        뜻이 계획서 밖에서 넓어진다. 기재하고 차단하지 않는다.
    """
    problems: list[str] = []
    notes: list[str] = []
    decl = read_declaration(camp_dir)
    if not decl:
        return problems, notes
    for node in assignments_of(decl):
        cells = [c for c in assigned_cells(decl, node) if c not in RESERVED_CELL_NAMES]
        if not lockset_observable(decl, node):
            if cells:
                notes.append(f"P6 ⓘ 관측 대상 밖(서브 인스턴스 소관 · 메인은 서브 인스턴스를 읽지 않는다) "
                             f"node={node}: {', '.join(cells)}")
            continue
        for cell in cells:
            lockset = camp_dir / "cells" / cell / "lockset.json"
            why = lockset_provenance_reason(lockset)
            if why is not None:
                problems.append(f"P6 {cell}: {why}")
                continue
            for extra in lockset_knob_source_reasons(lockset):
                notes.append(f"P6 ⓘ {cell}: {extra} — 노브 출처 어휘가 갈라지면 대조가 성립하지 않는다"
                             f"(기재 · 차단 ✗)")
    return problems, notes


def predicate_p6(camp_dir: Path) -> list[str]:
    """P6 — 관측 가능한 배정 셀 전수에 lockset 출처 표시가 있는가(적색 사유만). notes 는 p6_observations."""
    return p6_observations(camp_dir)[0]


def acceptance_predicates(camp_dir: Path) -> list[str]:
    """P4·P5·P6. 합격 판정(⑥)과 fetch 종료부가 부르고, **purge 게이트는 부르지 않는다**."""
    return (predicate_p4(camp_dir) + predicate_p5(camp_dir, brief_paths=discover_briefs(camp_dir))
            + predicate_p6(camp_dir))


def acceptance_notes(camp_dir: Path) -> list[str]:
    """합격 술어의 비차단 관측 줄(현재 P6 만). `--acceptance` 가 판정과 함께 출력한다."""
    return p6_observations(camp_dir)[1]


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
    # 셀 출처 칸(2026-09-14 · plan_26091407 §4.0). 측정 진입 precheck 와 P6 가 이 키를 읽는다 —
    # 뼈대에서 칸이 사라지면 새 셀은 전부 진입에서 멈추는데, 그 원인이 뼈대라는 사실은 멀리서 보인다.
    # 빈칸(<<FILL>>)으로 남아 있어야 한다: 기본값을 두면 "아무도 출처를 말하지 않았다"가 사라진다.
    lock_tpl = TEMPLATE / "cells/_cell/lockset.json"
    if lock_tpl.is_file():
        try:
            lock_doc = json.loads(lock_tpl.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            problems.append(f"뼈대 lockset.json 파손: {exc}")
        else:
            if not isinstance(lock_doc, dict) or lock_doc.get("provenance") != FILL:
                problems.append("뼈대 lockset.json 의 provenance 가 빈칸(<<FILL>>)이 아니다 — 출처는 "
                                "셀마다 사람이나 explorer 가 말해야 하며 뼈대가 기본값을 줄 수 없다")
            missing = [k for k in LOCKSET_KNOB_SOURCES if not (isinstance(lock_doc, dict) and k in lock_doc)]
            if missing:
                problems.append(f"뼈대 lockset.json 에 노브 출처 칸이 없다: {missing}")
            # 어휘 교차검증 — 뼈대의 `_*_enum` 배열은 저작자가 읽는 **안내 사본**이고 정본은 위 상수다.
            #   정적 파일끼리는 한쪽이 다른 쪽을 생성할 수 없으므로 교차검증이 차선이다
            #   (workflow.md §결정론 규율 · assert_band2_top_gitignore_parity 선례). 대조가 없으면
            #   뼈대 안내만 조용히 갈라지고, 저작자는 precheck 가 거부할 값을 안내대로 적는다.
            if isinstance(lock_doc, dict):
                expect = {"_provenance_enum": LOCKSET_PROVENANCE,
                          **{f"_{k}_enum": v for k, v in LOCKSET_KNOB_SOURCES.items()}}
                for key, allowed in expect.items():
                    if lock_doc.get(key) != list(allowed):
                        problems.append(f"뼈대 lockset.json 의 {key}={lock_doc.get(key)!r} 가 검증기 어휘 "
                                        f"{list(allowed)} 와 다르다 — 안내 사본이 정본에서 갈라졌다")
    # hint 사전승인 모양(2026-09-21 · O6) — 스키마의 승인 정의는 이 파일 상수의 사본이다. 정적 파일끼리는
    #   한쪽이 다른 쪽을 생성할 수 없으므로 교차검증이 차선이다(위 lockset `_*_enum` 선례). 대조가 없으면
    #   스키마만 조용히 갈라지고, writer 가 쓴 승인을 스키마가 거부하거나 그 반대가 된다.
    problems += _hint_schema_parity(TEMPLATE / "campaign.schema.json")
    if tpl.is_file():
        try:
            tpl_doc = json.loads(tpl.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            tpl_doc = None
        if not isinstance(tpl_doc, dict) or tpl_doc.get("hint_targets") != []:
            # 성장 배열은 빈 목록으로 출발한다 — 예시 승인이 인스턴스로 복사되면 사람이 하지 않은 승인이 된다.
            problems.append("뼈대 campaign.yaml 의 hint_targets 가 빈 목록이 아니다 — 예시 승인이 인스턴스로 "
                            "복사되면 사람이 하지 않은 승인이 기록된다")
    return problems


def _hint_schema_parity(schema_path: Path) -> list[str]:
    """스키마의 hint_target/hint_approval 정의가 검증기 상수와 같은 말을 하는가."""
    try:
        defs = json.loads(schema_path.read_text(encoding="utf-8")).get("definitions") or {}
    except (OSError, ValueError, AttributeError) as exc:
        return [f"뼈대 campaign.schema.json 파손: {exc}"]
    problems: list[str] = []
    target = defs.get("hint_target") or {}
    approval = defs.get("hint_approval") or {}
    for key in HINT_TARGET_RETIRED_KEYS:
        if key in (target.get("properties") or {}):
            problems.append(f"뼈대 스키마 hint_target 에 폐기 키 {key!r} 가 살아 있다")
    if target.get("additionalProperties") is not False:
        problems.append("뼈대 스키마 hint_target 의 additionalProperties 가 false 가 아니다 — 폐기 키가 조용히 통과한다")
    if ((target.get("properties") or {}).get("cells") or {}).get("minItems") != 1:
        problems.append("뼈대 스키마 hint_target.cells 의 minItems 가 1 이 아니다 — 빈 목록이 백지 승인이 된다")
    if list(approval.get("required") or []) != list(HINT_APPROVAL_FIELDS):
        problems.append(f"뼈대 스키마 hint_approval.required={approval.get('required')!r} 가 검증기 "
                        f"{list(HINT_APPROVAL_FIELDS)} 와 다르다 — 안내 사본이 정본에서 갈라졌다")
    enum = ((approval.get("properties") or {}).get("source") or {}).get("enum")
    if enum != list(HINT_APPROVAL_SOURCES):
        problems.append(f"뼈대 스키마 hint_approval.source.enum={enum!r} 가 검증기 {list(HINT_APPROVAL_SOURCES)} "
                        f"와 다르다 — 안내 사본이 정본에서 갈라졌다")
    return problems


def _write_brief(path: Path, doc: dict) -> Path:
    path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return path


def _selftest() -> int:
    """음성대조 포함. 라이브 트리는 깨끗할 때 아무것도 증명하지 않는다 — 양성이 발화해야 한다."""
    global TEMPLATE                       # 뼈대 음성대조가 사본 뼈대로 잠시 옮긴다(끝에서 복원)
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
    # hint 사전승인 픽스처(2026-09-21 · O6) — 옛 픽스처는 폐기된 arch 문법(`gb10-main-sim-h100`)과
    #   '비우면 전 셀' cells 를 썼다. 정상 선언은 완결된 승인 한 건을 든다(양성이 모양 전체를 지나게).
    _appr = {"approved_by": '사용자(발화 전사) — "cell-a 발행 승인"',
             "approved_utc": "2026-09-06T00:00:00Z", "source": HINT_APPROVAL_DEFAULT_SOURCE}

    def _ht(node: str = "main", cells=("cell-a",), **over) -> dict:
        return {"node_id": node, "cells": list(cells), "approval": dict(_appr, **over)}

    good = dict(base, id="camp-x", plan_ref="docs/plan/p.md", declared_utc="2026-09-06T00:00:00Z",
                nodes=[{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"}],
                matrix={"versions": ["0.18.0"], "models": ["gpt-oss-20b"]},
                assignments={"main": [{"cell": "cell-a"}]},
                budgets={"smoke_budget_overhead_mib": 12265, "ready_max_seconds": 600},
                control_variables={"model": "gpt-oss-20b", "vllm_version": "0.18.0",
                                   "topology": "single", "target_gpu": "H100"},
                hint_targets=[_ht()])

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
        # budgets.repeats(2026-09-14 · plan_26091407 §4.4) — full 정의의 반복 수. 생략은 정의값 3 이라
        #   합법이고, 3 미만 선언은 정의와 모순이라 스키마가 막는다. 소비자 배선은 단계 ④ 다.
        _b = dict(good["budgets"])
        ck("budgets.repeats 생략은 합법(소비자가 full 정의값 3 을 쓴다)", not write(dict(good, budgets=_b)))
        ck("budgets.repeats=3 선언 통과", not write(dict(good, budgets=dict(_b, repeats=3))))
        ck("★budgets.repeats<3 선언은 차단(반복 ≥3 이 full 의 정의다)",
           any("repeats" in p for p in write(dict(good, budgets=dict(_b, repeats=2)))))
        ck("통제변인 공란 차단",
           any("control_variables.model" in p
               for p in write(dict(good, control_variables=dict(good["control_variables"], model="")))))
        # ── hint 발행 사전승인 (2026-09-21 · plan_26092119 O6 · D8) ─────────────────────────
        #    옛 음성대조("옛 arch 문법 차단")는 arch 문법을 발행기 모듈에 물었다 — 모듈이 없으면 검사가
        #    조용히 꺼져 **이 음성대조만** 적색이 됐을 것이다(코드맵 H3). arch 는 이제 모양째 폐기다.
        _arch_left = write(dict(good, hint_targets=[dict(_ht(), arch="gb10-main-sim-h100")]))
        ck("★남은 arch 키는 차단하고 **이유**(D8 · 도구 파생)를 말한다(스키마의 '모르는 키' 만으로 두지 않는다)",
           any("arch" in p and "D8" in p for p in _arch_left)
           and any("SCHEMA_UNKNOWN_PROPERTY" in p and "arch" in p for p in _arch_left))
        ck("★옛 모양({arch, node_id, cells: []}) 그대로는 통과하지 못한다",
           len(write(dict(good, hint_targets=[{"arch": "gb10-main-sim-h100", "node_id": "main",
                                               "cells": []}]))) >= 3)
        ck("미지의 노드 참조 차단",
           any("nodes[] 에 없다" in p for p in write(dict(good, hint_targets=[_ht(node="ghost")]))))
        ck("★빈 cells 차단(옛 '비우면 전 셀' = 사전승인에선 백지 승인)",
           any("cells" in p and "minItems" in p for p in write(dict(good, hint_targets=[_ht(cells=())])))
           and any("백지 승인" in p for p in hint_target_reasons(dict(good, hint_targets=[_ht(cells=())]))))
        ck("★approval 자체가 없으면 차단(전사 없는 항목은 사전승인이 아니다)",
           any("approval" in p for p in write(dict(good, hint_targets=[
               {"node_id": "main", "cells": ["cell-a"]}]))))
        _half = dict(_ht()); _half["approval"] = {"approved_by": _appr["approved_by"], "source": "declaration-popup"}
        ck("★반쪽 승인 차단(approved_utc 부재)",
           any("approved_utc" in p for p in write(dict(good, hint_targets=[_half])))
           and hint_approval_reason(_half["approval"]) is not None)
        ck("★공백뿐인 approved_by 차단(스키마 minLength 는 통과하는 모양이라 python 수준이 잡는다)",
           any("approved_by" in p and "비었다" in p for p in write(dict(good, hint_targets=[_ht(approved_by="   ")]))))
        ck("★승인 필드의 <<FILL>> 차단",
           any(FILL in p for p in write(dict(good, hint_targets=[_ht(approved_by=FILL)])))
           and FILL in (hint_approval_reason(dict(_appr, approved_by=FILL)) or ""))
        ck("★주입 시각 모양이 아닌 approved_utc 차단(발행 페이로드는 이 모양만 옮긴다)",
           any("approved_utc" in p and "모양" in p
               for p in write(dict(good, hint_targets=[_ht(approved_utc="2026-09-06 00:00")]))))
        ck("★목록 밖 approval.source 차단",
           any("source" in p for p in write(dict(good, hint_targets=[_ht(source="chat")])))
           and "목록 밖" in (hint_approval_reason(dict(_appr, source="chat")) or ""))
        ck("★self_role=sub 인스턴스의 승인 기록 차단(발행은 메인 소관 · 파생 선언은 비워 보낸다)",
           any("self_role=sub" in p for p in write(dict(good, self_role="sub"))))
        ck("★음성대조: 완결 승인은 사유가 없다 · 기본 출처는 출처 목록 안이다",
           hint_approval_reason(_appr) is None and HINT_APPROVAL_DEFAULT_SOURCE in HINT_APPROVAL_SOURCES
           and not hint_target_reasons(good))
        _got = hint_approval_for(good, "cell-a")
        ck("★읽는 눈: 승인된 셀은 그 전사를 돌려준다(노드 필터 포함)",
           _got is not None and _got["approval"] == _appr and _got["node_id"] == "main"
           and hint_approval_for(good, "cell-a", "main") == _got)
        ck("★읽는 눈 음성대조: 승인 안 된 셀 · 다른 노드 필터는 None",
           hint_approval_for(good, "cell-b") is None and hint_approval_for(good, "cell-a", "subx") is None)
        ck("★읽는 눈 fail-closed: 승인 기록에 결함이 하나라도 있으면 어떤 승인도 읽지 않는다",
           hint_approval_for(dict(good, hint_targets=[_ht(), _ht(node="ghost")]), "cell-a") is None)
        _src = Path(__file__).read_text(encoding="utf-8")
        ck("★검증기가 발행기 모듈을 적재하지 않는다(순환 제거 · 부재 시 조용히 건너뛰던 H3 재발 방지)",
           ("hint" + "-publisher") not in _src and ("hint" + "_tag") not in _src)
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
        # 이 시험은 셀 입력만 본다 — 승인 픽스처(cell-a)가 배정 밖으로 밀려 섞이지 않게 비운다.
        ck("★음성대조: 빈칸을 채운 셀은 통과",
           not write(dict(good, assignments=_as(["cell-b"]), hint_targets=[])))
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
               hint_targets=[_ht(cells=("cell-zzz",))]))))
        ck("★다른 노드에 배정된 셀을 이 노드의 승인으로 적으면 차단(배정의 노드가 승인의 노드다)",
           any("cell-c" in p and "assignments['main']" in p
               for p in write(dict(good, nodes=nodes2, assignments=two, hint_targets=[_ht(cells=("cell-c",))]))))
        ck("★한 셀의 승인은 하나다(두 승인 사건이 같은 셀을 들면 차단)",
           any("한 셀의 승인은 하나다" in p for p in write(dict(good,
               hint_targets=[_ht(), _ht(approved_utc="2026-09-07T00:00:00Z")]))))
        ck("★음성대조: 같은 노드의 서로 다른 셀을 두 승인 사건으로 적으면 통과(항목 하나 = 승인 사건 하나)",
           not write(dict(good, assignments=_as(["cell-a", "cell-b"]),
                          hint_targets=[_ht(), _ht(cells=("cell-b",), approved_utc="2026-09-07T00:00:00Z",
                                                   source="publish-popup")])))
        ck("★음성대조: 두 노드가 각자 자기 배정 셀을 승인하면 통과",
           not write(dict(good, nodes=nodes2, assignments=two,
                          hint_targets=[_ht(), _ht(node="subx", cells=("cell-c",))])))
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

        # ── R0: 측정노드 어휘(`cluster`) → 진행노드 어휘 해소 (2026-09-11 · plan_26091108) ──
        #    이 넷이 없으면 multi 캠페인의 purge 게이트가 구조적으로 열리지 않는다.
        decl2(two_nodes, {"main": [{"cell": "cell-a"}], "sub": [{"cell": "cell-c"}]})
        (c2 / "evidence_pointers.json").write_text(json.dumps({"pointers": [
            {"kind": "certificate", "path": "CLAUDE.md", "cell_id": "cell-a",
             "node_id": "cluster"}]}), encoding="utf-8")
        ck("★R0 파생축 cluster 는 선언 노드 어디에도 bench 진행표가 없으면 검출",
           any("cluster" in x and "선언 노드 어디에도" in x for x in predicate_p2(c2)))
        (c2 / "phases" / "main").mkdir(parents=True, exist_ok=True)
        (c2 / "phases" / "main" / "bench.status.json").write_text(json.dumps({
            "schema_version": 1, "node_id": "main", "phase": "bench", "state": "running",
            "proof": {"predicate": "리포트 실재", "ok": False, "source": "sweep_index.json"}}),
            encoding="utf-8")
        ck("★R0 진행표가 있어도 done+ok 가 아니면 반영하지 않은 것이다",
           any("cluster" in x and "반영하지 않는다" in x for x in predicate_p2(c2)))
        (c2 / "phases" / "main" / "bench.status.json").write_text(json.dumps({
            "schema_version": 1, "node_id": "main", "phase": "bench", "state": "done",
            "proof": {"predicate": "리포트 실재", "ok": True, "source": "docs/benchmark/r.md"}}),
            encoding="utf-8")
        ck("★R0 구동 노드 하나가 반영하면 통과(Ray 워커의 없는 bench 를 요구하지 않는다)",
           not any(x.startswith("P2 cluster") for x in predicate_p2(c2)))
        (c2 / "evidence_pointers.json").write_text(json.dumps({"pointers": [
            {"kind": "certificate", "path": "CLAUDE.md", "cell_id": "cell-a",
             "node_id": "maiin"}]}), encoding="utf-8")
        ck("★R0 어느 어휘에도 없는 태그는 오타로 지목한다(조용한 통과 ✗)",
           any("maiin" in x and "해소할 수 없다" in x for x in predicate_p2(c2)))
        # 교차검증 — 이 파일의 파생축 목록은 거울이다. 정본(producer)의 리터럴이 사라지면
        # 여기 사본이 조용히 틀린 말을 하게 되므로, 리터럴 실재를 시험이 붙잡는다.
        _prod = REPO_ROOT / _DERIVED_NODE_PRODUCER[0]
        ck("★R0 파생축 리터럴이 producer 에 실재한다(거울 갈라짐 방지)",
           _prod.is_file() and _DERIVED_NODE_PRODUCER[1] in _prod.read_text(encoding="utf-8"))
        (c2 / "phases" / "main" / "bench.status.json").unlink()

        decl2(two_nodes, {"main": [{"cell": "cell-a"}], "sub": [{"cell": "cell-c"}]})
        (c2 / "evidence_pointers.json").unlink()
        ck("★P4 는 메인 시각이 없으면 그 사실을 말한다",
           any("시각이 하나도 없다" in x for x in predicate_p4(c2)))
        (c2 / "phases" / "main").mkdir(parents=True, exist_ok=True)
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

        # ── P6 셀 출처 표시 (2026-09-14 · plan_26091407 §4.0) ──────────────────────────────
        #    양성(표시 부재·무효가 발화)과 음성대조(손작성 표시는 통과 · 관측 불가 셀은 요구 ✗)를 둘 다
        #    둔다 — 음성대조가 없으면 "모든 셀을 막는 술어" 도 초록으로 보인다.
        c6 = Path(tmp) / "camp-6"
        for _c in ("p6-a", "p6-b", "p6-s"):
            (c6 / "cells" / _c).mkdir(parents=True)
            (c6 / "cells" / _c / "config.yaml").write_text(f"cell_id: {_c}\n", encoding="utf-8")
        _decl6 = dict(good, id="camp-6", hint_targets=[],
                      nodes=[{"node_id": "main", "role": "main", "topology": "single", "hw": "gb10"},
                             {"node_id": "subx", "role": "sub", "topology": "single", "hw": "gb10"}],
                      assignments={"main": [{"cell": "p6-a"}, {"cell": "p6-b"}],
                                   "subx": [{"cell": "p6-s"}]})
        (c6 / "campaign.yaml").write_text(json.dumps(_decl6, ensure_ascii=False), encoding="utf-8")

        def _lock(cell: str, doc: dict | None) -> None:
            path = c6 / "cells" / cell / "lockset.json"
            if doc is None:
                path.unlink(missing_ok=True)
            else:
                path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")

        _lock("p6-a", None)
        _lock("p6-b", {"id": "p6-b", "provenance": "hand-authored"})
        p6 = predicate_p6(c6)
        ck("★P6 배정 셀의 lockset 부재를 검출(종전 검증기는 부재를 continue 로 넘겼다)",
           any(x.startswith("P6 p6-a") and "부재" in x for x in p6))
        ck("★P6 음성대조: hand-authored 표시는 통과한다(손작성은 금지가 아니라 표시 대상)",
           not any(x.startswith("P6 p6-b") for x in p6))
        ck("★P6 메인 인스턴스는 서브 배정 셀의 lockset 을 요구하지 않는다(관측 불가 · 영구 적색 ✗)",
           not any(x.startswith("P6 p6-s") for x in p6))
        ck("★P6 관측 대상 밖 셀은 조용히 건너뛰지 않고 이름으로 남는다(공허 통과 ✗ · notes)",
           any("관측 대상 밖" in x and "p6-s" in x and "node=subx" in x for x in acceptance_notes(c6))
           and not any("p6-s" in x for x in acceptance_predicates(c6)))
        ck("관측 가능성 판정은 한 함수다(메인 인스턴스 · role=sub 노드 = 관측 불가)",
           not lockset_observable(_decl6, "subx") and lockset_observable(_decl6, "main")
           and lockset_observable(_decl6, None)
           and lockset_observable(dict(_decl6, self_role="sub"), "subx"))
        # 뼈대를 복사만 한 lockset — 빈칸은 목록 밖 값으로 거부된다.
        _tpl_lock = json.loads((TEMPLATE / "cells/_cell/lockset.json").read_text(encoding="utf-8"))
        _lock("p6-a", dict(_tpl_lock, id="p6-a"))
        ck("★P6 뼈대 복사본(provenance=<<FILL>>)은 출처를 말하지 않은 것이다",
           any(x.startswith("P6 p6-a") and "무효" in x for x in predicate_p6(c6)))
        _lock("p6-a", {"id": "p6-a", "provenance": "explorer"})
        ck("★P6 목록 밖 provenance 를 검출(어휘 갈라짐)",
           any(x.startswith("P6 p6-a") and "무효" in x for x in predicate_p6(c6)))
        _lock("p6-a", {"id": "p6-a"})
        ck("★P6 provenance 키 자체가 없으면 검출",
           any(x.startswith("P6 p6-a") and "부재" in x for x in predicate_p6(c6)))
        _lock("p6-a", {"id": "p6-a", "provenance": "explorer-phase2", "batch_source": "guess",
                       "gmu_source": None, "kv_source": "measured-clamp"})
        _p6x, _p6n = p6_observations(c6)
        ck("★P6 노브 출처가 목록 밖이면 **기재**한다(null 은 '아직 안 정함'이라 사유 아님)",
           any("batch_source" in x for x in _p6n) and not any("gmu_source" in x for x in _p6n)
           and not any("kv_source" in x for x in _p6n))
        ck("★P6 음성대조: 노브 출처 이탈은 적색이 아니다(합격 정의 = provenance 표시 · E2E ⑥ 의미 고정)",
           not any(x.startswith("P6 p6-a") for x in _p6x))
        _lock("p6-a", {"id": "p6-a", "provenance": "explorer-phase2", "batch_source": "kv-fit-measured",
                       "gmu_source": "target_gmu", "kv_source": "measured-clamp"})
        ck("★P6 음성대조: 표시가 전수 성립하면 P6 는 아무 말도 하지 않는다", not predicate_p6(c6))
        (c6 / "campaign.yaml").write_text(json.dumps(dict(
            _decl6, self_role="sub", nodes=[_decl6["nodes"][1]],
            assignments={"subx": [{"cell": "p6-s"}]}), ensure_ascii=False), encoding="utf-8")
        ck("★P6 서브 자기 인스턴스(self_role=sub)에서는 자기 배정 셀을 요구한다",
           any(x.startswith("P6 p6-s") for x in predicate_p6(c6)))
        (c6 / "campaign.yaml").write_text(json.dumps(_decl6, ensure_ascii=False), encoding="utf-8")
        _lock("p6-a", None)
        ck("★P6 는 acceptance 에 배선돼 있다(fetch 종료부가 실제로 부르는 함수)",
           any(x.startswith("P6 p6-a") for x in acceptance_predicates(c6)))
        ck("★P6 는 purge 선행조건이 아니다(표시 누락 셀이 다음 캠페인을 영원히 막지 않는다)",
           not any(x.startswith("P6") for x in instance_predicates(c6)))
        ck("★precheck 와 P6 가 같은 사유 함수를 쓴다(hand-authored 통과 · 부재 거부)",
           lockset_provenance_reason(c6 / "cells" / "p6-b" / "lockset.json") is None
           and lockset_provenance_reason(c6 / "cells" / "p6-a" / "lockset.json") is not None)

        # 뼈대 음성대조 — 사본 뼈대에서 출처 칸에 기본값을 박으면 뼈대 검증이 발화한다.
        import shutil as _shutil
        _saved_tpl = TEMPLATE
        _tpl_copy = Path(tmp) / "tpl"
        _shutil.copytree(_saved_tpl, _tpl_copy)
        _tl = _tpl_copy / "cells/_cell/lockset.json"
        try:
            TEMPLATE = _tpl_copy
            ck("사본 뼈대는 그대로 완결이다(음성대조의 기준선)", not validate_template())
            _tl.write_text(json.dumps(dict(_tpl_lock, provenance="hand-authored"), ensure_ascii=False),
                           encoding="utf-8")
            ck("★뼈대 lockset 이 출처 기본값을 가지면 발화한다(빈칸이 곧 계약)",
               any("provenance" in x for x in validate_template()))
            _tl.write_text(json.dumps({k: v for k, v in _tpl_lock.items() if k != "kv_source"},
                                      ensure_ascii=False), encoding="utf-8")
            ck("★뼈대 lockset 에서 노브 출처 칸이 사라지면 발화한다",
               any("kv_source" in x for x in validate_template()))
            _tl.write_text(json.dumps(dict(_tpl_lock, _provenance_enum=["explorer", "manual"],
                                           _gmu_source_enum=["log"]), ensure_ascii=False),
                           encoding="utf-8")
            _drift = validate_template()
            ck("★뼈대 안내 어휘(_*_enum)가 검증기 상수에서 갈라지면 발화한다(교차검증)",
               any("_provenance_enum" in x for x in _drift) and any("_gmu_source_enum" in x for x in _drift)
               and not any("_kv_source_enum" in x for x in _drift))
            _tl.write_text(json.dumps(_tpl_lock, ensure_ascii=False), encoding="utf-8")
            # hint 사전승인 모양의 교차검증(2026-09-21 · O6) — 스키마 사본이 상수에서 갈라지면 발화한다.
            _ts = _tpl_copy / "campaign.schema.json"
            _schema_ok = json.loads(_ts.read_text(encoding="utf-8"))
            ck("★기준선: 사본 스키마의 승인 정의는 검증기 상수와 같다", not _hint_schema_parity(_ts))
            _bad = json.loads(json.dumps(_schema_ok))
            _bad["definitions"]["hint_approval"]["properties"]["source"]["enum"] = ["declaration-popup"]
            _bad["definitions"]["hint_target"]["properties"]["arch"] = {"type": "string"}
            del _bad["definitions"]["hint_target"]["properties"]["cells"]["minItems"]
            _ts.write_text(json.dumps(_bad, ensure_ascii=False), encoding="utf-8")
            _hdrift = validate_template()
            ck("★스키마 승인 출처 enum 이 상수에서 갈라지면 발화한다",
               any("source.enum" in x for x in _hdrift))
            ck("★스키마에 폐기 키 arch 가 되살아나면 발화한다", any("폐기 키 'arch'" in x for x in _hdrift))
            ck("★스키마 cells 의 minItems 가 사라지면 발화한다(빈 목록 = 백지 승인)",
               any("minItems" in x for x in _hdrift))
            _ts.write_text(json.dumps(_schema_ok, ensure_ascii=False), encoding="utf-8")
            _ty = _tpl_copy / "campaign.yaml"
            _tyd = json.loads(_ty.read_text(encoding="utf-8"))
            _ty.write_text(json.dumps(dict(_tyd, hint_targets=[_ht()]), ensure_ascii=False), encoding="utf-8")
            ck("★뼈대 선언에 예시 승인이 들면 발화한다(사람이 하지 않은 승인이 인스턴스로 복사된다)",
               any("hint_targets" in x for x in validate_template()))
            _ty.write_text(json.dumps(_tyd, ensure_ascii=False), encoding="utf-8")
            ck("원복하면 사본 뼈대는 다시 완결이다", not validate_template())
        finally:
            TEMPLATE = _saved_tpl
    print("[campaign_template_validator] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--campaign", help="검증할 campaign.yaml 경로")
    ap.add_argument("--instance", help="검증할 campaigns/<id>/ 디렉터리")
    ap.add_argument("--template", action="store_true", help="뼈대 자체를 검증")
    ap.add_argument("--acceptance", metavar="INSTANCE",
                    help="합격 술어 P4(동시 착수 타임라인)·P5(서브 자기저작)·P6(셀 출처 표시)만 "
                         "판정한다 — purge 게이트와 분리된 자리다(진행을 막지 않는다)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.selftest:
            return _selftest()
        problems: list[str] = []
        notes: list[str] = []
        if a.template or not (a.campaign or a.instance or a.acceptance):
            problems += validate_template()
        if a.campaign:
            problems += validate_campaign(Path(a.campaign))
        if a.instance:
            problems += validate_instance(Path(a.instance))
        if a.acceptance:
            problems += acceptance_predicates(Path(a.acceptance))
            notes += acceptance_notes(Path(a.acceptance))
    except CampaignContractFailure as exc:
        print(f"[campaign_template_validator] FAIL {exc}", file=sys.stderr)
        return 1
    # 비차단 관측 줄 — 판정과 무관하게 먼저 낸다(적색 여부가 관측 대상 밖 셀의 존재를 가리지 않게).
    for n in notes:
        print(f"  {n}")
    if problems:
        print(f"[campaign_template_validator] FAIL ({len(problems)})", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1
    print("[campaign_template_validator] PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
