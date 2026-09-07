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
    """completion_gate 가 이미 구현한 Draft-07 부분집합을 그대로 쓴다(두 번째 엔진을 만들지 않는다)."""
    path = REPO_ROOT / ".claude/policies/runtime/completion_gate.py"
    spec = importlib.util.spec_from_file_location("_campaign_completion_gate", path)
    gate = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gate)
    return [f"{where}: {v}" for v in gate.validate_against_schema(doc, schema)]


def find_fill_placeholders(text: str) -> list[int]:
    """`<<FILL>>` 이 남은 행 번호. 모르는 값을 그럴듯하게 채우는 것보다 **모른다고 멈추는** 것이 싸다."""
    return [i for i, line in enumerate(text.splitlines(), 1) if FILL in line]


def validate_campaign(campaign_path: Path, *, strict_paths: bool = True) -> list[str]:
    """캠페인 선언 1건을 검증하고 **위반 목록**을 돌려준다(빈 리스트 = 통과)."""
    problems: list[str] = []
    raw = campaign_path.read_text(encoding="utf-8") if campaign_path.is_file() else None
    if raw is None:
        return [f"campaign 선언 부재: {campaign_path}"]

    fills = find_fill_placeholders(raw)
    if fills:
        problems.append(f"{campaign_path.name}: 플레이스홀더 {FILL} 가 {len(fills)}행에 남아 있다(행 {fills[:10]})")

    try:
        doc = json.loads(raw)
    except ValueError as exc:
        return problems + [f"{campaign_path.name}: JSON 파손 — {exc}"]
    doc.pop("_howto", None)

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
        if target.get("node_id") not in node_ids:
            problems.append(f"hint_targets[{i}].node_id {target.get('node_id')!r} 가 nodes[] 에 없다")

    # order ↔ cells 실재
    # ★ 2026-09-06: 종전에는 config.yaml 의 **실재**만 물었다. 그런데 스캐폴드가 남기는 틀
    #   `cells/_cell/config.yaml` 도 실재하는 파일이라, order 를 디렉터리 목록에서 파생하면
    #   틀이 그대로 실행 순서에 섞여 들어가고 검증기가 통과시켰다(내가 실제로 그렇게 했다).
    #   scaffold 주석은 "빈칸이 남으면 검증기가 잡는다"고 말했지만 그 검사는 campaign.yaml
    #   에만 걸려 있었다 — 계약과 집행이 갈라져 있었던 것이다. 여기서 붙인다.
    cells_dir = campaign_path.parent / "cells"
    for cell in doc.get("order", []):
        if cell in RESERVED_CELL_NAMES:
            problems.append(f"order 에 틀 디렉터리 {cell!r} 가 들어 있다 — 틀은 실행 대상이 아니다")
            continue
        cfg = cells_dir / cell / "config.yaml"
        if strict_paths and not cfg.is_file():
            problems.append(f"order 의 셀 {cell!r} 입력이 없다: {_rel(cfg)}")
            continue
        if not strict_paths:
            continue
        # 셀 입력의 빈칸도 계약이다 — 모르는 값을 그럴듯하게 채우지 말라는 규칙(campaigns/README)
        # 은 검사가 있어야 규칙이다.
        for name in ("config.yaml", "lockset.json"):
            f = cells_dir / cell / name
            if not f.is_file():
                continue
            rows = find_fill_placeholders(f.read_text(encoding="utf-8"))
            if rows:
                problems.append(f"셀 {cell!r} 의 {name} 에 {FILL} 가 {len(rows)}행 남아 있다"
                                f"(행 {rows[:5]})")
    return problems


def validate_instance(camp_dir: Path) -> list[str]:
    """인스턴스 하나를 검증한다 — 선언 + 셀 + phase proof."""
    problems = validate_campaign(camp_dir / "campaign.yaml")
    for status in sorted(camp_dir.glob("phases/*/*.status.json")):
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
_SWEEP_TO_CELL = {
    "measured": "measured", "serve_failed": "serve_failed", "build_failed": "build_failed",
    "void": "void", "pending": "pending",
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


def predicate_p2(camp_dir: Path) -> list[str]:
    """P2 — 증거가 도착한 (노드, 셀)의 phase 가 그 사실을 반영하는가.

    벤치 증거(인증서·리포트)가 실재하는데 그 노드의 bench phase 가 `done` 이 아니면, 진행표가
    증거보다 낡은 것이다. 진행표를 믿고 재개하면 이미 잰 셀을 다시 잰다.
    """
    problems: list[str] = []
    ep = camp_dir / "evidence_pointers.json"
    if not ep.is_file():
        return problems
    try:
        doc = json.loads(ep.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return problems
    bench_nodes: dict = {}
    for ptr in (doc.get("pointers") or []):
        if not isinstance(ptr, dict):
            continue
        if str(ptr.get("kind")) in ("certificate", "bench_report"):
            node = ptr.get("node_id")
            if isinstance(node, str) and node:
                bench_nodes.setdefault(node, []).append(str(ptr.get("path")))
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
                order=["cell-a"],
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
        ck("부재 셀 차단", any("입력이 없다" in p for p in write(dict(good, order=["cell-missing"]))))
        # ★ 2026-09-06: order 를 cells/ 디렉터리 목록에서 파생하면 스캐폴드가 남긴 틀
        #   `_cell` 이 그대로 섞인다. 틀의 config.yaml 은 **실재하므로** 실재 검사만으로는
        #   통과했다(실제로 통과시켰다 — 이 시험이 그 재발을 막는다).
        (camp / "cells" / "_cell").mkdir(parents=True)
        (camp / "cells" / "_cell" / "config.yaml").write_text(
            f"target_model:\n  path: {FILL}\n", encoding="utf-8")
        ck("★틀 디렉터리가 order 에 들어가면 차단",
           any("틀은 실행 대상이 아니다" in p for p in write(dict(good, order=["_cell"]))))
        # 셀 입력의 빈칸도 계약이다. `_cell` 은 이름으로 먼저 걸리므로 **다른 이름**으로 시험한다
        #   — 같은 픽스처가 두 규칙을 겸하면 어느 쪽이 발화했는지 모른다.
        (camp / "cells" / "cell-b").mkdir(parents=True)
        (camp / "cells" / "cell-b" / "config.yaml").write_text(
            f"target_model:\n  path: {FILL}\n", encoding="utf-8")
        ck("★셀 입력에 빈칸이 남으면 차단",
           any("config.yaml 에" in p and FILL in p for p in write(dict(good, order=["cell-b"]))))
        (camp / "cells" / "cell-b" / "lockset.json").write_text(
            '{"id": "' + FILL + '"}\n', encoding="utf-8")
        (camp / "cells" / "cell-b" / "config.yaml").write_text("target_model:\n  path: /x\n",
                                                               encoding="utf-8")
        ck("★lockset 의 빈칸도 차단",
           any("lockset.json 에" in p for p in write(dict(good, order=["cell-b"]))))
        # 음성대조: 빈칸을 채우면 같은 셀이 통과한다(과잉차단 아님)
        (camp / "cells" / "cell-b" / "lockset.json").write_text('{"id": "cell-b"}\n',
                                                                encoding="utf-8")
        ck("★음성대조: 빈칸을 채운 셀은 통과", not write(dict(good, order=["cell-b"])))
        ck("스키마 미지 필드 차단", any("campaign.yaml" in p for p in write(dict(good, surprise=1))))

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
    print("[campaign_template_validator] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--campaign", help="검증할 campaign.yaml 경로")
    ap.add_argument("--instance", help="검증할 campaigns/<id>/ 디렉터리")
    ap.add_argument("--template", action="store_true", help="뼈대 자체를 검증")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.selftest:
            return _selftest()
        problems: list[str] = []
        if a.template or not (a.campaign or a.instance):
            problems += validate_template()
        if a.campaign:
            problems += validate_campaign(Path(a.campaign))
        if a.instance:
            problems += validate_instance(Path(a.instance))
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
