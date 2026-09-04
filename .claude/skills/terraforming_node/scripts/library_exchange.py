#!/usr/bin/env python3
"""library_exchange.py — 그라운딩 교환(자료구조화 포맷)의 **결정론 판정기** (헌법 §불변식 B).

정본 서술: `.claude/skills/terraforming_node/SKILL.md` §2.7.8.
메시지 계약: `.claude/skills/terraforming_node/sub_node/library-exchange.schema.json`.

**분업이 설계의 전부다** — *누락은 기계가, 거짓은 사람이.*
서브가 "이 근거로 이렇게 빌드했다"고 말할 때, **그 근거가 실제로 반출된 항목인지·빠진 인용은
없는지**는 결정론으로 답할 수 있다. 반면 "그 근거가 이 결정을 정말 뒷받침하는가"는 사람이 읽어야
한다. 이 파일은 앞엣것만 한다 — 뒤엣것을 흉내 내면 판정이 확률론이 되고 게이트가 무의미해진다.
(원리 차용 = `docs/report/ttsc-evidence-graph-principles-implementation.md` §1·§5. 기법이 아니라
**claim–reference 의무 그래프와 세 질문**만 이식한다.)

세 질문(증거그래프 §Evaluation 차용):

  1. **Resolution**       — 모든 인용이 반출된 항목 정확히 하나로 해소되는가
  2. **Host eligibility** — 인용을 단 결정이 인용 의무 모집단인가
  3. **Coverage**         — 모든 채택된 결정이 최소 하나의 인용을 갖는가 (0 = **omission**)

  + **Freshness** — 인용 시점 digest 가 반출 시점 digest 와 같은가(`requireReview` 차용).
    인용된 것이 바뀌면 인용은 만료된다 — 조용히 유효한 척하지 않는다.

**지식 비대칭은 판정에도 새겨져 있다.** 도서관(`__llm-wiki`)·사서(`wiki-desk`)는 메인 단독이고
서브로 복제되지 않는다. 그래서 반출은 **경로+앵커+digest+발췌**이며, 발췌가 예산을 넘으면
`EXPORT_EXCEEDS_EXCERPT_BUDGET` 로 거부한다 — 반출을 복제로 바꾸는 것이 비대칭을 깨는 실제 경로다.

CLI:
    library_exchange.py validate --file <message.json>
    library_exchange.py gate --request <req.json> --export <exp.json> --attestation <att.json>
    library_exchange.py receive (--report <r.json> | --agent-control-result <res.json>
                                | --invoke-request <req.json>)
                                [--exchange-dir <d> | --request/--export/--attestation]
    library_exchange.py --self-test

`receive` 가 **수신 경로의 정문**이다. `gate` 는 판정만 하고 "언제 물어야 하는가"를 모른다 —
그래서 2026-08-22 E2E 에서 gate 를 돌린 주체는 파이프라인이 아니라 사람이었다(testlog §4.5).
`receive` 는 서브 리포트를 받아 **인용 의무가 있는지 스스로 판정**하고, 의무가 있는데 교환 3메시지가
없으면 `GROUNDING_EXCHANGE_ABSENT` 로 거부한다 — 미인용이 조용히 통과하는 경로를 닫는다.

종료코드: 0 = 통과 · 2 = 사용오류 · 5 = 게이트 위반(fail-closed)

벽시계를 읽지 않는다. 서브 디스크를 읽지 않는다(메시지만 본다 — push-attestation 보존).
"""
import argparse
import importlib.util
import json
import os
import sys

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_GATE_VIOLATION = 5

_HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(_HERE, "..", "..", "..", ".."))
SCHEMA_PATH = os.path.join(_HERE, "..", "sub_node", "library-exchange.schema.json")
# ★ 2026-09-01 (audit_26090109 ②) — task-report 스키마는 존재했으나 저장소 전체에
#   **로더가 0개**였다. 그 결과 receive() 는 dict 이기만 하면 무엇이든 통과시켰고,
#   `phase` 오타 한 글자·`status` 대문자 하나로 `grounding_required` 가 False 가 되어
#   **헌법 불변식 B 의 유일한 기계 집행점이 열렸다**. 검증기(completion_gate)도 스키마도
#   이미 있었고 **연결만 없었다**.
TASK_REPORT_SCHEMA_PATH = os.path.join(_HERE, "..", "sub_node", "task-report.schema.json")
_task_report_schema_cache = None


def load_task_report_schema(path=None):
    global _task_report_schema_cache
    if path is None and _task_report_schema_cache is not None:
        return _task_report_schema_cache
    with open(path or TASK_REPORT_SCHEMA_PATH, "r", encoding="utf-8") as handle:
        schema = json.load(handle)
    if path is None:
        _task_report_schema_cache = schema
    return schema
COMPLETION_GATE_SCRIPT = os.path.join(
    REPO_ROOT, ".claude", "policies", "runtime", "completion_gate.py")

SUPPORTED_SCHEMA_VERSION = 1

KIND_REQUEST = "library.citation.request"
KIND_EXPORT = "library.resolution.export"
KIND_ATTESTATION = "library.citation.attestation"

# 종류별 **배타 블록**. 스키마의 additionalProperties:false 는 "이 계약에 없는 키"만 막고,
# "이 종류에 없어야 할 블록"은 못 막는다(Draft-07 부분집합에 not/oneOf 가 없다). 여기서 닫는다 —
# 한 오브젝트가 request 이면서 export 이면 어느 평면의 판정을 적용할지 모호해진다.
KIND_BLOCKS = {
    KIND_REQUEST: {"claim", "query"},
    KIND_EXPORT: {"resolution", "references"},
    KIND_ATTESTATION: {"claim", "citations", "decision"},
}
_ALL_BLOCKS = set().union(*KIND_BLOCKS.values())

# 인용 의무 모집단(host eligible). `informational` 은 질의만 하고 결정하지 않으므로 의무 밖이다.
GROUNDED_CLAIM_KINDS = ("build-decision", "serve-decision", "version-pin", "patch-slot")

# ── 수신 계약(§2.7.8 정문) — 인용 의무가 언제 발생하는가 ─────────────────────────────────
# `phase` 는 "무엇을 했나", `status` 는 "그것을 채택했나"다. 둘 다여야 의무가 선다.
#   inspect = 카나리(정체성 self-report) — 빌드·서빙을 바꾸지 않으므로 의무 밖.
#   completed 아닌 것(working/input-required/failed) = 아직 채택이 아니다 — 유보에 인용을 요구하면
#   서브가 막혔다고 정직하게 보고하는 경로가 오히려 벌을 받는다(gate 의 accepted=false 규약과 동형).
GROUNDED_REPORT_PHASES = ("config", "build", "serve")
ACCEPTING_REPORT_STATUSES = ("completed",)

# 발췌 예산 — **이 파일이 단일 소유자**다. 생산자(`library_relay.py`)가 여기서 import 한다.
#
# ⚠ 2026-09-04 교정: 종전 주석은 이 값을 *"이 파일에서만 쓰는 국소 상수(매직넘버 — 정당 칸)"* 라고
#   자칭했으나 사실이 아니었다. 생산자에도 같은 개념이 `EXCERPT_CHARS` 로 앉아 있었고, 2026-09-03
#   P4 에서 1200→6000 상향을 **생산자에만** 적용해 두 자리가 갈라졌다. 그 결과 6000자로 잘라 낸
#   반출을 같은 저장소의 이 게이트가 `EXPORT_EXCEEDS_EXCERPT_BUDGET` 로 **fail-closed 거부**했다
#   (2026-09-04 실증 rc=5). 판정표의 "같은 개념이 두 곳 이상에 손으로 적힌 값" = **결함** 칸이다.
#
# 값(6000)의 근거는 생산자에 있던 실측 서사 그대로다 — 발췌는 읽기 보조가 아니라 **서브 결정의
# 유일한 근거**이며, 1200 에서 잘린 발췌가 서브의 정직한 유보 → KV 클램프 미채택 → 워치독 사살로
# 이어졌다(`policy:KV_ABSOLUTE_CLAMP_PORTABILITY`). 상한을 두는 이유는 여전히 있다(서브 컨텍스트
# 비용) — 없애지 않고 근거 있는 값으로 올린다. 비대칭 규약(반출은 도서관 복제가 아니다)도 유지된다.
EXCERPT_MAX_CHARS = 6000


class _Violations(list):
    def add(self, code, message, where=None):
        self.append({"code": code, "message": message, "where": where})


# --------------------------------------------------------------------------------------
# 스키마 shape 검증 — 정본 검증기 재사용(두 번째 손저작 구현을 만들지 않는다)
# --------------------------------------------------------------------------------------
_cgate_module = None


def _cgate():
    """`completion_gate.validate_against_schema`(Draft-07 부분집합 일반 검증기)를 재적재한다.

    서브 배포에서는 동거 사본으로, 메인 레이아웃에서는 `.claude/policies/runtime/` 정본 경로에서
    적재한다 — `hint_tag.py::_cgate` 와 같은 두-레이아웃 패턴. 없으면 **모른다고 말하고 실패**한다
    (자체 검증기로 조용히 갈아타지 않는다 — 그러면 두 판정기가 갈린다).
    """
    global _cgate_module
    if _cgate_module is None:
        try:
            import completion_gate as _cg
        except ModuleNotFoundError:
            if not os.path.isfile(COMPLETION_GATE_SCRIPT):
                raise SystemExit(
                    "[library-exchange] FAIL: completion_gate.py 를 찾을 수 없다 "
                    f"(동거 사본 ✗ · {COMPLETION_GATE_SCRIPT} ✗) — shape 검증기를 대체하지 않는다.")
            spec = importlib.util.spec_from_file_location(
                "completion_gate", COMPLETION_GATE_SCRIPT)
            _cg = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(_cg)
        _cgate_module = _cg
    return _cgate_module


_schema_cache = None


def load_schema(path=None):
    global _schema_cache
    if path is None and _schema_cache is not None:
        return _schema_cache
    target = path or SCHEMA_PATH
    with open(target, "r", encoding="utf-8") as handle:
        schema = json.load(handle)
    if path is None:
        _schema_cache = schema
    return schema


def validate_message(obj, schema=None):
    """한 메시지의 형태를 검증한다. → violations list (빈 리스트 = 통과)."""
    violations = _Violations()
    if not isinstance(obj, dict):
        violations.add("SCHEMA_TYPE_MISMATCH", "메시지는 JSON object 여야 한다.")
        return violations

    schema = schema or load_schema()
    for code, message in _cgate().validate_against_schema(obj, schema):
        violations.add(code, message)

    version = obj.get("schema_version")
    if isinstance(version, int) and version > SUPPORTED_SCHEMA_VERSION:
        violations.add(
            "SCHEMA_VERSION_UNSUPPORTED",
            "schema_version=%d 는 판정기가 아는 판(%d)보다 크다 — fail-closed."
            % (version, SUPPORTED_SCHEMA_VERSION), where="schema_version")

    kind = obj.get("kind")
    if kind in KIND_BLOCKS:
        for block in sorted(_ALL_BLOCKS - KIND_BLOCKS[kind]):
            if block in obj:
                violations.add(
                    "KIND_BLOCK_FOREIGN",
                    "kind=%s 오브젝트에 %r 블록이 있다 — 한 메시지는 한 종류만 담는다."
                    % (kind, block), where=block)
    return violations


# --------------------------------------------------------------------------------------
# 세 질문 + freshness
# --------------------------------------------------------------------------------------
def gate(request, export, attestation, schema=None):
    """→ {"questions": {...}, "violations": [...], "cited": [...]} — 절대 raise 하지 않는다."""
    violations = _Violations()
    result = {
        "questions": {"resolution": None, "host_eligibility": None, "coverage": None,
                      "freshness": None},
        "violations": violations,
        "cited": [],
    }

    for label, obj, expected_kind in (
            ("request", request, KIND_REQUEST),
            ("export", export, KIND_EXPORT),
            ("attestation", attestation, KIND_ATTESTATION)):
        for v in validate_message(obj, schema):
            v["where"] = "%s.%s" % (label, v["where"]) if v.get("where") else label
            violations.append(v)
        if isinstance(obj, dict) and obj.get("kind") != expected_kind:
            violations.add("KIND_MISMATCH",
                           "%s 자리에 kind=%r 가 왔다(기대 %r)."
                           % (label, obj.get("kind"), expected_kind), where=label)
    if violations:
        return result                      # 형태가 깨진 입력에 의미 판정을 얹지 않는다

    # ── 교환 동일성: 셋이 같은 exchange_id 를 되돌려야 추적이 성립한다 ──
    ids = {label: obj.get("exchange_id")
           for label, obj in (("request", request), ("export", export),
                              ("attestation", attestation))}
    if len(set(ids.values())) != 1:
        violations.add("EXCHANGE_ID_MISMATCH",
                       "exchange_id 가 갈렸다: %r — 어느 반출이 어느 결정을 뒷받침했는지 "
                       "추적 불가." % (ids,))
        return result

    req_claim = request.get("claim") or {}
    att_claim = attestation.get("claim") or {}
    if req_claim.get("claim_id") != att_claim.get("claim_id"):
        violations.add("CLAIM_ID_MISMATCH",
                       "요청 claim_id=%r 와 귀속 claim_id=%r 가 다르다."
                       % (req_claim.get("claim_id"), att_claim.get("claim_id")))

    # ── 반출 항목 색인 + 발췌 예산(비대칭 유지) ──
    refs = {}
    for index, ref in enumerate(export.get("references") or []):
        ref_id = ref.get("ref_id")
        if ref_id in refs:
            violations.add("EXPORT_DUPLICATE_REF_ID",
                           "반출에 ref_id=%r 가 두 번 있다 — 인용이 어느 항목으로 해소되는지 "
                           "모호해진다." % (ref_id,), where="export.references[%d]" % index)
        refs[ref_id] = ref
        excerpt = ref.get("excerpt") or ""
        if len(excerpt) > EXCERPT_MAX_CHARS:
            violations.add("EXPORT_EXCEEDS_EXCERPT_BUDGET",
                           "발췌 %d자 > 예산 %d자 — 반출은 발췌이지 도서관 복제가 아니다"
                           "(지식 비대칭 유지)." % (len(excerpt), EXCERPT_MAX_CHARS),
                           where="export.references[%d].excerpt" % index)

    citations = attestation.get("citations") or []
    decision = attestation.get("decision") or {}
    accepted = bool(decision.get("accepted"))
    claim_kind = att_claim.get("kind")

    # ── 질문 2: Host eligibility ──
    #   채택된 결정만 인용 의무를 진다. informational 은 의무 밖이므로, 그것으로 결정을
    #   채택하면 의무를 우회하는 셈이다 — 그 우회를 여기서 닫는다.
    if accepted and claim_kind not in GROUNDED_CLAIM_KINDS:
        violations.add("HOST_INELIGIBLE",
                       "claim.kind=%r 로 결정을 채택했다 — 인용 의무 모집단은 %r 이다."
                       % (claim_kind, list(GROUNDED_CLAIM_KINDS)), where="attestation.claim.kind")
    result["questions"]["host_eligibility"] = not any(
        v["code"] == "HOST_INELIGIBLE" for v in violations)

    # ── 질문 3: Coverage — 0 인용 = omission ──
    if accepted and not citations:
        status = (export.get("resolution") or {}).get("status")
        extra = ""
        if status in ("unresolved", "refused"):
            extra = (" 사서가 status=%s 로 답했다면 올바른 결과는 채택이 아니라 "
                     "accepted=false + HITL 에스컬레이션이다." % status)
        violations.add("GROUNDING_OMISSION",
                       "채택된 결정에 인용이 0건이다 — 인용 없는 결정은 '필요했다'는 증거가 "
                       "없다(누락은 기계가 잡는다)." + extra, where="attestation.citations")
    result["questions"]["coverage"] = not any(
        v["code"] == "GROUNDING_OMISSION" for v in violations)

    # ── 질문 1: Resolution + Freshness ──
    for index, cite in enumerate(citations):
        ref_id = cite.get("ref_id")
        where = "attestation.citations[%d]" % index
        ref = refs.get(ref_id)
        if ref is None:
            violations.add("CITATION_UNRESOLVED",
                           "인용 ref_id=%r 가 반출 목록에 없다 — 반출되지 않은 것을 인용하면 "
                           "'무엇'도 증명하지 못한다." % (ref_id,), where=where)
            continue
        if cite.get("digest") != ref.get("digest"):
            violations.add("CITATION_STALE",
                           "인용 digest=%r 가 반출 digest=%r 와 다르다 — 인용된 것이 바뀌었다"
                           "(인용 만료)." % (cite.get("digest"), ref.get("digest")), where=where)
            continue
        result["cited"].append(ref_id)
    result["questions"]["resolution"] = not any(
        v["code"] == "CITATION_UNRESOLVED" for v in violations)
    result["questions"]["freshness"] = not any(
        v["code"] == "CITATION_STALE" for v in violations)
    return result


# --------------------------------------------------------------------------------------
# 수신 — 서브 task-report 정문 (W-5 · 자동 호출 배선)
# --------------------------------------------------------------------------------------
def grounding_required(report):
    """이 리포트가 인용 의무 모집단인가. → (bool, 사유문자열)

    **판정 입력은 리포트 자신이다** — 호출자가 "이번엔 그라운딩 필요 없어"라고 말할 자리를 두지
    않는다. 그 자리를 두면 의무는 선택이 되고, 선택인 게이트는 게이트가 아니다.
    """
    if not isinstance(report, dict):
        return False, "report-not-object"
    phase = report.get("phase")
    status = report.get("status")
    if phase not in GROUNDED_REPORT_PHASES:
        return False, "phase:%s(비-결정 위상)" % (phase,)
    if status not in ACCEPTING_REPORT_STATUSES:
        return False, "status:%s(미채택 — 유보·실패에는 인용 의무가 없다)" % (status,)
    return True, "phase:%s+status:%s" % (phase, status)


def receive(report, request=None, export=None, attestation=None, schema=None):
    """서브 리포트 1건 수신 판정. → dict (절대 raise 하지 않는다)

    반환 키: accepted · grounding_required · obligation_reason · gate · violations · provenance
    """
    violations = _Violations()
    out = {
        "accepted": False,
        "grounding_required": None,
        "obligation_reason": None,
        "gate": None,
        "violations": violations,
    }

    if not isinstance(report, dict):
        violations.add("REPORT_UNREADABLE",
                       "task-report 가 JSON object 가 아니다 — 수신은 fail-closed 다"
                       "(형태를 모르면 의무 여부도 모른다).")
        return out

    # ★ 형태를 확인하기 전에는 의무를 판정하지 않는다(2026-09-01 · ②). `grounding_required`
    #   는 `phase`·`status` 를 읽어 인용 의무를 정하는데, 그 두 필드가 스키마 밖 값이면
    #   의무 판정 자체가 무의미하다 — 오타가 곧 면제가 된다.
    try:
        _tr_schema = load_task_report_schema(schema if isinstance(schema, str) else None)
        _tr_problems = list(_cgate().validate_against_schema(report, _tr_schema))
    except Exception as exc:                       # fail-loud: 검증할 수 없으면 통과시키지 않는다
        violations.add("REPORT_SCHEMA_UNAVAILABLE",
                       "task-report 스키마를 적용할 수 없다(%s: %s) — 수신은 fail-closed 다."
                       % (type(exc).__name__, exc))
        return out
    if _tr_problems:
        for _code, _msg in _tr_problems:
            violations.add("REPORT_SCHEMA_INVALID",
                           "task-report 스키마 위반[%s]: %s" % (_code, _msg))
        return out

    required, reason = grounding_required(report)
    out["grounding_required"] = required
    out["obligation_reason"] = reason

    triple = (request, export, attestation)
    supplied = [t is not None for t in triple]

    if not required:
        if any(supplied):
            # 의무 밖이어도 3메시지를 냈다면 판정해 준다 — 낸 것을 안 보는 것은 침묵이다.
            if not all(supplied):
                violations.add("EXCHANGE_TRIPLE_INCOMPLETE",
                               "교환 3메시지 중 일부만 제출됐다 — 셋은 한 묶음이다.",
                               where="exchange")
                return out
            out["gate"] = _gate_payload(gate(request, export, attestation, schema))
            violations.extend(out["gate"]["violations"])
        out["accepted"] = not violations
        return out

    if not all(supplied):
        violations.add(
            "GROUNDING_EXCHANGE_ABSENT",
            "인용 의무가 있는 리포트(%s)인데 교환 3메시지가 %s — 인용 없는 결정은 거짓이 아니라 "
            "**누락**이고, 누락은 기계가 fail-closed 로 잡는다(헌법 §불변식 B). 서브가 "
            "library.citation.request 를 내고 메인 사서가 반출한 뒤 서브가 attestation 을 "
            "되돌려야 수신이 성립한다." % (reason, "없다" if not any(supplied) else "불완전하다"),
            where="exchange")
        return out

    out["gate"] = _gate_payload(gate(request, export, attestation, schema))
    violations.extend(out["gate"]["violations"])
    if violations:
        return out

    # ── 결속: 이 교환이 **이 리포트의** 결정을 뒷받침하는가 ──
    #   묶지 않으면 다른 노드·다른 결정의 통과한 교환을 재사용해 게이트를 우회할 수 있다
    #   (M-2 의 execution_approval 재인가와 같은 형태의 결함 — W-8).
    att_node = (attestation or {}).get("node_id")
    if att_node != report.get("node_id"):
        violations.add("EXCHANGE_NODE_MISMATCH",
                       "attestation.node_id=%r 와 리포트 node_id=%r 가 다르다 — 다른 노드의 "
                       "교환으로 이 리포트를 인가할 수 없다."
                       % (att_node, report.get("node_id")), where="attestation.node_id")
    accepted_decl = bool(((attestation or {}).get("decision") or {}).get("accepted"))
    if not accepted_decl:
        violations.add("DECISION_ACCEPTANCE_MISMATCH",
                       "리포트는 status=completed(채택)인데 attestation.decision.accepted=false 다 "
                       "— 자기귀속이 서로 어긋난다.", where="attestation.decision.accepted")

    out["accepted"] = not violations
    return out


def _gate_payload(result):
    return {"questions": result["questions"], "cited": result["cited"],
            "violations": list(result["violations"])}


# --------------------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------------------
def _read_json(path, label):
    if not os.path.isfile(path):
        print("[library-exchange] FAIL: %s 부재: %s" % (label, path), file=sys.stderr)
        raise SystemExit(EXIT_USAGE)
    try:
        with open(path, "r", encoding="utf-8") as handle:
            return json.load(handle)
    except Exception as exc:                                   # noqa: BLE001
        print("[library-exchange] FAIL: %s JSON 파싱 실패(%s): %s" % (label, path, exc),
              file=sys.stderr)
        raise SystemExit(EXIT_USAGE)


def _report(payload, violations):
    print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
    return EXIT_GATE_VIOLATION if violations else EXIT_OK


def cmd_validate(args):
    obj = _read_json(args.file, "message")
    violations = validate_message(obj)
    return _report({"kind": obj.get("kind") if isinstance(obj, dict) else None,
                    "violations": list(violations)}, violations)


def cmd_gate(args):
    request = _read_json(args.request, "request")
    export = _read_json(args.export, "export")
    attestation = _read_json(args.attestation, "attestation")
    result = gate(request, export, attestation)
    payload = {"questions": result["questions"], "cited": result["cited"],
               "violations": list(result["violations"])}
    return _report(payload, result["violations"])


EXCHANGE_FILENAMES = {"request": "request.json", "export": "export.json",
                      "attestation": "attestation.json"}

AGENT_CONTROL_SCRIPT = os.path.join(
    REPO_ROOT, ".claude", "policies", "runtime", "agent_control.py")


def _extract_report(text, label):
    """agent 텍스트에서 task-report JSON 1개를 꺼낸다. 실패는 None(호출부가 fail-closed 처리).

    comms.md 계약은 *"stdout 으로 JSON 1개, raw 로그 금지"* 다. 그래도 앞뒤에 잡문이 붙는 실측이
    있으므로 **첫 `{` ~ 마지막 `}`** 한 번만 관대하게 잘라 본다 — 그 이상 추측하지 않는다
    (여러 후보를 뒤지기 시작하면 판정기가 파서가 된다).
    """
    if not isinstance(text, str):
        return None
    try:
        return json.loads(text)
    except ValueError:
        pass
    lo, hi = text.find("{"), text.rfind("}")
    if lo < 0 or hi <= lo:
        return None
    try:
        return json.loads(text[lo:hi + 1])
    except ValueError:
        return None


def _resolve_exchange(args):
    """--exchange-dir 또는 개별 3플래그 → (request, export, attestation). 미제출은 None."""
    if args.exchange_dir:
        got = {}
        for key, name in EXCHANGE_FILENAMES.items():
            path = os.path.join(args.exchange_dir, name)
            got[key] = _read_json(path, key) if os.path.isfile(path) else None
        return got["request"], got["export"], got["attestation"]
    return (_read_json(args.request, "request") if args.request else None,
            _read_json(args.export, "export") if args.export else None,
            _read_json(args.attestation, "attestation") if args.attestation else None)


def cmd_receive(args):
    sources = [bool(args.report), bool(args.agent_control_result), bool(args.invoke_request)]
    if sum(sources) != 1:
        print("[library-exchange] FAIL: --report · --agent-control-result · --invoke-request "
              "중 정확히 하나를 준다.", file=sys.stderr)
        return EXIT_USAGE

    provenance = {}
    if args.report:
        report = _read_json(args.report, "report")
        provenance = {"report_source": "task-report", "report_path": args.report}
    else:
        if args.invoke_request:
            # 위임과 수신을 **한 명령으로 묶는다** — 두 단계로 두면 두 번째를 건너뛸 수 있고,
            # 건너뛸 수 있는 게이트는 게이트가 아니다(2026-08-22 E2E 가 실증한 형태).
            # 전송은 헌법 소유(provider-neutral orchestrator)에 남기고 여기서 만들지 않는다.
            import subprocess
            if not os.path.isfile(AGENT_CONTROL_SCRIPT):
                print("[library-exchange] FAIL: agent_control.py 부재: %s" % AGENT_CONTROL_SCRIPT,
                      file=sys.stderr)
                return EXIT_USAGE
            proc = subprocess.run(
                [sys.executable, AGENT_CONTROL_SCRIPT, "invoke", "--request", args.invoke_request],
                capture_output=True, text=True)
            try:
                result = json.loads(proc.stdout)
            except ValueError:
                print("[library-exchange] FAIL: agent_control 산출이 JSON 이 아니다"
                      "(exit=%d)." % proc.returncode, file=sys.stderr)
                if proc.stderr:
                    print(proc.stderr, file=sys.stderr)
                return EXIT_GATE_VIOLATION
            provenance = {"report_source": "invoked",
                          "agent_control_exit": proc.returncode,
                          "invoke_request_path": args.invoke_request}
        else:
            result = _read_json(args.agent_control_result, "agent-control-result")
            provenance = {"report_source": "agent-control-result",
                          "result_path": args.agent_control_result}
        if not isinstance(result, dict):
            report = None
        else:
            provenance["agent_control_status"] = result.get("status")
            report = _extract_report(result.get("output"), "output")

    exchange = _resolve_exchange(args)
    out = receive(report, *exchange)
    payload = {"accepted": out["accepted"],
               "grounding_required": out["grounding_required"],
               "obligation_reason": out["obligation_reason"],
               "gate": out["gate"],
               "provenance": provenance,
               "violations": list(out["violations"])}
    print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
    return EXIT_OK if out["accepted"] else EXIT_GATE_VIOLATION


# --------------------------------------------------------------------------------------
# 자기검사
# --------------------------------------------------------------------------------------
def _self_test():
    failures = []
    checks = []                    # 총계는 **파생**한다 — 손으로 적은 총계는 케이스를 더해도
                                   # 조용히 옛 수를 보고한다(4종 판정표 하드코딩 '결함' 칸).

    def chk(name, cond, detail=""):
        checks.append(name)
        print("%s %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " — %s" % detail))
        if not cond:
            failures.append(name)

    def codes(violations):
        return sorted({v["code"] for v in violations})

    base = {"schema_version": 1, "exchange_id": "x-1", "node_id": "sub", "topology": "single"}
    req = dict(base, kind=KIND_REQUEST,
               claim={"claim_id": "c-1", "kind": "serve-decision",
                      "statement": "gpt-oss 를 triton MoE 백엔드로 서빙한다"},
               query={"terms": ["moe-backend", "sm_121a"]})
    exp = dict(base, kind=KIND_EXPORT,
               resolution={"status": "resolved", "librarian": "wiki-desk"},
               references=[{"ref_id": "r-1", "path": "docs/testlog/t.md",
                            "digest": "a" * 64, "authority": "execution-truth",
                            "excerpt": "CUTLASS JIT OOM → --moe-backend triton"}])
    att = dict(base, kind=KIND_ATTESTATION,
               claim=req["claim"],
               citations=[{"ref_id": "r-1", "digest": "a" * 64, "reason": "동일 arch 실측"}],
               decision={"accepted": True, "notes": "R2 자체이식 채택 — 동일 arch 실측 인용"})

    chk("정상 3메시지 shape 통과",
        not validate_message(req) and not validate_message(exp) and not validate_message(att))

    r = gate(req, exp, att)
    chk("정상 교환 → 위반 0 · 네 질문 모두 참 · cited=[r-1]",
        not r["violations"] and all(r["questions"].values()) and r["cited"] == ["r-1"],
        (codes(r["violations"]), r["questions"]))

    # ── coverage(omission) ──
    r = gate(req, exp, dict(att, citations=[]))
    chk("★ 인용 0 + 채택 → GROUNDING_OMISSION (불변식 B 하드게이트)",
        codes(r["violations"]) == ["GROUNDING_OMISSION"] and r["questions"]["coverage"] is False,
        codes(r["violations"]))

    r = gate(req, exp, dict(att, citations=[], decision={"accepted": False}))
    chk("인용 0 + 미채택(유보) → 위반 없음(정직한 결과)", not r["violations"],
        codes(r["violations"]))

    exp_unresolved = dict(exp, resolution={"status": "unresolved", "reason": "도서관 공백"},
                          references=[])
    r = gate(req, exp_unresolved, dict(att, citations=[]))
    chk("도서관 공백인데 채택 → omission + HITL 안내 문구",
        codes(r["violations"]) == ["GROUNDING_OMISSION"]
        and "HITL" in r["violations"][0]["message"], r["violations"])

    # ── resolution ──
    r = gate(req, exp, dict(att, citations=[{"ref_id": "r-99", "digest": "a" * 64}]))
    chk("반출에 없는 ref 인용 → CITATION_UNRESOLVED",
        codes(r["violations"]) == ["CITATION_UNRESOLVED"]
        and r["questions"]["resolution"] is False, codes(r["violations"]))

    dup = dict(exp, references=exp["references"] + [dict(exp["references"][0])])
    r = gate(req, dup, att)
    chk("반출 ref_id 중복 → EXPORT_DUPLICATE_REF_ID",
        "EXPORT_DUPLICATE_REF_ID" in codes(r["violations"]), codes(r["violations"]))

    # ── freshness ──
    r = gate(req, exp, dict(att, citations=[{"ref_id": "r-1", "digest": "c" * 64}]))
    chk("digest 불일치 → CITATION_STALE(인용 만료)",
        codes(r["violations"]) == ["CITATION_STALE"] and r["questions"]["freshness"] is False,
        codes(r["violations"]))

    # ── host eligibility ──
    info_claim = {"claim_id": "c-1", "kind": "informational", "statement": "궁금해서 물었다"}
    r = gate(dict(req, claim=info_claim), exp, dict(att, claim=info_claim))
    chk("informational claim 으로 결정 채택 → HOST_INELIGIBLE",
        codes(r["violations"]) == ["HOST_INELIGIBLE"]
        and r["questions"]["host_eligibility"] is False, codes(r["violations"]))

    # ── 교환 동일성 ──
    r = gate(req, dict(exp, exchange_id="x-2"), att)
    chk("exchange_id 갈림 → EXCHANGE_ID_MISMATCH(의미 판정 중단)",
        codes(r["violations"]) == ["EXCHANGE_ID_MISMATCH"], codes(r["violations"]))

    r = gate(req, exp, dict(att, claim=dict(att["claim"], claim_id="c-9")))
    chk("claim_id 갈림 → CLAIM_ID_MISMATCH",
        "CLAIM_ID_MISMATCH" in codes(r["violations"]), codes(r["violations"]))

    # ── 비대칭(발췌 예산) ──
    fat = dict(exp, references=[dict(exp["references"][0], excerpt="가" * (EXCERPT_MAX_CHARS + 1))])
    r = gate(req, fat, att)
    chk("★ 발췌 예산 초과 → EXPORT_EXCEEDS_EXCERPT_BUDGET (반출 ≠ 도서관 복제)",
        "EXPORT_EXCEEDS_EXCERPT_BUDGET" in codes(r["violations"]), codes(r["violations"]))

    # ── shape ──
    v = validate_message(dict(base, kind=KIND_REQUEST, claim=req["claim"]))
    chk("request 에 query 누락 → 필수키 위반",
        any(c.startswith("SCHEMA_MISSING_REQUIRED_KEY") for c in codes(v)), codes(v))

    v = validate_message(dict(req, references=exp["references"]))
    chk("한 메시지에 두 종류 블록 → KIND_BLOCK_FOREIGN",
        "KIND_BLOCK_FOREIGN" in codes(v), codes(v))

    v = validate_message(dict(req, schema_version=99))
    chk("미래 schema_version → fail-closed",
        "SCHEMA_VERSION_UNSUPPORTED" in codes(v), codes(v))

    v = validate_message(dict(req, topology="auto"))
    chk("topology 어휘 밖 → enum 위반(브랜치 추론 ✗)",
        any(c.startswith("SCHEMA_ENUM_VIOLATION") for c in codes(v)), codes(v))

    r1 = json.dumps({"q": gate(req, exp, att)["questions"]}, sort_keys=True)
    r2 = json.dumps({"q": gate(req, exp, att)["questions"]}, sort_keys=True)
    chk("결정론(동일 입력 → 동일 출력)", r1 == r2)

    # ── 수신 정문(W-5) ────────────────────────────────────────────────────────
    def rep(**kw):
        base_rep = {"task_id": "t-1", "context_id": "c-1", "turn": 1, "phase": "serve",
                    "status": "completed", "node_id": "sub",
                    "self_verification": {"checks_run": ["schema_valid"]}}
        base_rep.update(kw)
        return base_rep

    chk("의무 판정: serve+completed → 의무 있음", grounding_required(rep())[0] is True)
    chk("의무 판정: inspect(카나리) → 의무 없음",
        grounding_required(rep(phase="inspect"))[0] is False)
    chk("의무 판정: serve+input-required(유보) → 의무 없음",
        grounding_required(rep(status="input-required"))[0] is False)

    rc = receive(rep())
    chk("★ 의무 있는데 교환 3메시지 부재 → GROUNDING_EXCHANGE_ABSENT (미인용 조용한 통과 차단)",
        rc["accepted"] is False
        and codes(rc["violations"]) == ["GROUNDING_EXCHANGE_ABSENT"], codes(rc["violations"]))

    rc = receive(rep(), req, exp, None)
    chk("교환 3메시지 중 일부만 → 거부(셋은 한 묶음)",
        rc["accepted"] is False
        and codes(rc["violations"]) == ["GROUNDING_EXCHANGE_ABSENT"], codes(rc["violations"]))

    rc = receive(rep(), req, exp, att)
    chk("정상 리포트 + 통과 교환 → accepted",
        rc["accepted"] is True and rc["gate"]["cited"] == ["r-1"], rc)

    rc = receive(rep(), req, exp, dict(att, citations=[]))
    chk("리포트는 completed 인데 인용 0 → 수신 거부(omission 전파)",
        rc["accepted"] is False and "GROUNDING_OMISSION" in codes(rc["violations"]),
        codes(rc["violations"]))

    rc = receive(rep(node_id="sub-other"), req, exp, att)
    chk("★ 다른 노드의 교환으로 인가 시도 → EXCHANGE_NODE_MISMATCH(교환 재사용 차단)",
        rc["accepted"] is False and codes(rc["violations"]) == ["EXCHANGE_NODE_MISMATCH"],
        codes(rc["violations"]))

    rc = receive(rep(), req, exp, dict(att, decision={"accepted": False}))
    chk("completed 인데 attestation 은 미채택 → DECISION_ACCEPTANCE_MISMATCH",
        rc["accepted"] is False
        and "DECISION_ACCEPTANCE_MISMATCH" in codes(rc["violations"]), codes(rc["violations"]))

    rc = receive(rep(phase="inspect"))
    chk("카나리(inspect) 는 교환 없이도 수신 성립(의무 밖)",
        rc["accepted"] is True and rc["grounding_required"] is False, rc)

    rc = receive("not-an-object")
    chk("리포트가 object 가 아님 → REPORT_UNREADABLE(fail-closed)",
        rc["accepted"] is False and codes(rc["violations"]) == ["REPORT_UNREADABLE"],
        codes(rc["violations"]))

    # ── ② 회귀: task-report 스키마 하드게이트 (2026-09-01 · audit_26090109 ②) ──────────
    # 종전 receive() 는 dict 이기만 하면 통과시켰다. 그래서 `phase` 오타 한 글자로
    # grounding_required 가 False 가 되고 accepted=True 가 나왔다 — **불변식 B 의 유일한
    # 기계 집행점이 오타로 열린다**. 아래 케이스들이 그 문을 닫아 둔다.
    _ok_report = {"task_id": "t-1", "node_id": "sub", "context_id": "c-1", "turn": 1,
                  "phase": "build", "status": "completed",
                  "self_verification": {"checks_run": ["import", "smoke"]}}
    rc = receive(dict(_ok_report))
    chk("★스키마 적합 build/completed → 의무 성립(GROUNDING_EXCHANGE_ABSENT)",
        rc["grounding_required"] is True
        and codes(rc["violations"]) == ["GROUNDING_EXCHANGE_ABSENT"],
        codes(rc["violations"]))
    for _label, _mut in (("phase 오타", {"phase": "buld"}),
                         ("status 대소문자", {"status": "Completed"}),
                         ("turn=0(minimum 위반)", {"turn": 0}),
                         ("계약 밖 키", {"extra": "x"})):
        _bad = dict(_ok_report); _bad.update(_mut)
        rc = receive(_bad)
        chk("★스키마 위반(%s) → REPORT_SCHEMA_INVALID · 의무 판정 안 함" % _label,
            rc["accepted"] is False
            and rc["grounding_required"] is None
            and codes(rc["violations"]) == ["REPORT_SCHEMA_INVALID"],
            codes(rc["violations"]))
    rc = receive({})
    chk("★빈 오브젝트 → REPORT_SCHEMA_INVALID(형태 모르면 의무도 모른다)",
        rc["accepted"] is False and codes(rc["violations"]) == ["REPORT_SCHEMA_INVALID"],
        codes(rc["violations"]))

    chk("agent 텍스트에서 리포트 추출(앞뒤 잡문 1회 관대)",
        _extract_report('말머리 {"phase": "serve"} 말꼬리', "x") == {"phase": "serve"})
    chk("추출 불가는 None(호출부 fail-closed)", _extract_report("no json here", "x") is None)

    total = len(checks)
    print("--- %d/%d PASS" % (total - len(failures), total))
    return 0 if not failures else 1


def main():
    ap = argparse.ArgumentParser(description="그라운딩 교환(불변식 B)의 결정론 판정기")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    v = sub.add_parser("validate", help="메시지 1개의 형태 검증")
    v.add_argument("--file", required=True)
    g = sub.add_parser("gate", help="request/export/attestation 3종의 세 질문 판정")
    g.add_argument("--request", required=True)
    g.add_argument("--export", required=True)
    g.add_argument("--attestation", required=True)
    r = sub.add_parser("receive", help="서브 task-report 수신 정문(그라운딩 의무 자동 판정·fail-closed)")
    src = r.add_mutually_exclusive_group(required=True)
    src.add_argument("--report", help="서브 task-report JSON 경로")
    src.add_argument("--agent-control-result", dest="agent_control_result",
                     help="agent_control.py invoke 산출 JSON(그 output 에서 리포트를 꺼낸다)")
    src.add_argument("--invoke-request", dest="invoke_request",
                     help="agent-control request JSON — 위임 실행과 수신 판정을 한 명령으로 묶는다")
    r.add_argument("--exchange-dir", dest="exchange_dir",
                   help="request.json/export.json/attestation.json 이 든 디렉터리")
    r.add_argument("--request")
    r.add_argument("--export")
    r.add_argument("--attestation")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()
    if args.cmd == "validate":
        return cmd_validate(args)
    if args.cmd == "gate":
        return cmd_gate(args)
    if args.cmd == "receive":
        return cmd_receive(args)
    ap.print_help()
    return EXIT_USAGE


if __name__ == "__main__":
    sys.exit(main())
