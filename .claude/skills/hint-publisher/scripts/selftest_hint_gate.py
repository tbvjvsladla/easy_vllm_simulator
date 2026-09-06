#!/usr/bin/env python3
"""selftest_hint_gate.py — hint 발행 게이트의 **절단선**을 계약으로 고정한다.

2026-09-06(plan_26090616 Q7/Q8) 사용자 결정으로 발행 정책이 바뀌었다:

    hint 태그의 롤 = **토큰노믹스 정책**. 필수는 "여정 정보" 하나뿐이고, 그 밖의 **부재는
    차단 사유가 아니라 기재 대상**이다. 차단은 **양성 검출**일 때만이다.

정책 문장은 시간이 지나면 코드와 갈라진다. 이 파일은 그 문장을 **실행 가능한 시험**으로 붙잡는다 —
절단선 양쪽에 각각 입력을 넣어 통과/차단을 실제로 확인한다. 라이브 트리는 깨끗할 때 아무것도
증명하지 않으므로, 여기서는 **양성이 실제로 발화하는지**가 시험의 본체다(역-오라클 회피).

  실행: selftest_hint_gate.py            (rc 0=PASS · 1=FAIL)
"""
from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FAILURES: list[str] = []


def _load(name: str):
    sys.path.insert(0, str(HERE))
    spec = importlib.util.spec_from_file_location(f"_gate_{name}", HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ck(label: str, cond: bool) -> None:
    print(("  PASS " if cond else "  FAIL ") + label)
    if not cond:
        FAILURES.append(label)


def main() -> int:
    ht = _load("hint_tag")
    hc = _load("hint_collect")
    rb = _load("render_bench_section")

    print("── ① 통과해야 하는 것: 부재는 기재하고 발행한다 ──")
    # 결손 사유코드는 **코드표에 등재돼 있어야** 한다. 코드만 뱉고 뜻이 없으면 수신자는 그 줄을
    # 읽고도 무엇이 없는지 모른다.
    for code in ("HINT_MISSING_CERTIFICATE", "HINT_MISSING_BENCH_REPORT",
                 "HINT_MISSING_SWEEP_LEVELS", "HINT_MISSING_LITE",
                 "HINT_MISSING_SLAVE_ATTESTATION", "HINT_MISSING_ENV_SHAPE",
                 "HINT_MISSING_SUB_TRIPLET", "HINT_MISSING_PII_TERMS",
                 "HINT_MISSING_MEASURED_NODE"):
        ck(f"결손 사유코드 등재: {code}", code in hc.MISSING_CODES and bool(hc.MISSING_CODES[code]))

    body = hc.render_item3({}, "(부재)", bench_section="", missing=["HINT_MISSING_CERTIFICATE"])
    ck("인증서 부재로 **죽지 않는다**(절을 렌더한다)", "# 3. 벤치 결과" in body)
    ck("부재 사유가 본문에 적힌다", "HINT_MISSING_CERTIFICATE" in body)
    ck("부재를 성능 판정으로 읽지 말라고 적는다", "판정되지 않았다" in body)

    filled = hc.render_item3({"decode_tps_conc1": 45.28}, "cert.yaml",
                             bench_section="## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)",
                             missing=[])
    ck("결손 0 도 **명시**한다(침묵 ✗)", "결손 없음" in filled)
    ck("벤치 절이 항목3 안에 실린다", "부하 스윕 곡선 — 동시성별" in filled)

    print("── ② 차단해야 하는 것: 양성 검출 ──")
    # (a) PII 매치 — env 형상화 뒤에도 남으면 조용히 덧칠하지 않고 죽는다
    raised = None
    try:
        hc.env_shape_template("PEER_ENDPOINT=192.168.0.99\n")   # pii-scan-fixture
    except SystemExit as exc:
        raised = exc
    ck("★PII 매치는 차단(형상화 뒤 잔존)", raised is not None and raised.code != 0)

    # (b) arch 문법 — 노드 축 없는 옛 이름은 통과하지 못한다
    ck("★옛 arch 문법 차단", ht.arch_violation("gb10-sim-h100") == "HINT_ARCH_NODE_AXIS_ABSENT")
    ck("★형태 위반 차단", ht.arch_violation("GB10-main-x") == "HINT_ARCH_SHAPE_VIOLATION")
    ck("arch 부재 차단", ht.arch_violation("") == "HINT_ARCH_ABSENT")
    ck("정상 arch 3종 통과", all(ht.arch_violation(a) is None for a in
                              ("gb10-main-sim-h100", "gb10-sub-sim-h100", "gb10x2-cluster-sim-h100")))

    # (c) 벤치 표 위조 — 한 숫자만 손대도 재렌더 대조가 잡는다
    report = "\n".join(["## 부하 스윕 곡선 (client-load · reload 없음)", "",
                        rb.REPORT_HEADER, "|---|---|---|---|---|---|---|",
                        "| 1 ★판정점 | 45.28 | 45.30 | 226.70 | 160.76 | 21.54 | 16/0 |", ""])
    parsed = rb.parse_report(report)
    good = rb.render(parsed, "r.md")
    ck("★숫자 손댐 검출(재렌더 diff)", rb.extract_section(good.replace("45.28", "99.99") + "\n## 끝\n")
       != rb.extract_section(good + "\n## 끝\n"))
    raised2 = None
    try:
        rb.parse_report("# 표가 없는 문서\n")
    except rb.BenchSectionFailure as exc:
        raised2 = exc
    ck("★표 부재는 빈 표가 아니라 FAIL", raised2 is not None)

    print("── ③ 절단선 자체: 선언된 부재만 통과한다 ──")
    ck("절단선 매핑에 인증서 부재가 있다",
       ht._MISSING_CODE_FOR_ABSENCE.get("HINT_EVIDENCE_CERTIFICATE_REF_ABSENT") == "HINT_MISSING_CERTIFICATE")
    ck("★work-manifest 부재는 면제하지 않는다(발행자 자신의 증거)",
       "HINT_EVIDENCE_MANIFEST_REF_ABSENT" not in ht._MISSING_CODE_FOR_ABSENCE)

    saved = ht.declared_missing_of
    try:
        ht.declared_missing_of = lambda tag: frozenset({"HINT_MISSING_CERTIFICATE"})
        ck("선언된 부재 → 통과",
           ht.absence_is_declared("t", ["HINT_EVIDENCE_CERTIFICATE_REF_ABSENT"]) is True)
        ck("★선언되지 않은 부재 → 차단",
           ht.absence_is_declared("t", ["HINT_EVIDENCE_MANIFEST_REF_ABSENT"]) is False)
        ck("★부분 선언은 통과가 아니다(하나라도 미선언이면 차단)",
           ht.absence_is_declared("t", ["HINT_EVIDENCE_CERTIFICATE_REF_ABSENT",
                                        "HINT_EVIDENCE_MANIFEST_REF_ABSENT"]) is False)
        ht.declared_missing_of = lambda tag: frozenset()
        ck("★선언이 비면 면제 없음", ht.absence_is_declared("t", ["HINT_EVIDENCE_CERTIFICATE_REF_ABSENT"]) is False)
        ck("★사유 목록이 비면 면제 없음(공허 통과 ✗)", ht.absence_is_declared("t", []) is False)
    finally:
        ht.declared_missing_of = saved

    print("── ③b A/B 성질 분리: 서빙 성공은 무조건 차단 · 정량지표는 선언 기반 ──")
    import tempfile
    ck("정량지표 결손 리더가 존재한다", callable(getattr(ht, "_payload_declared_missing", None)))
    ck("★경로 없음 → 빈 집합(읽지 못한 것을 선언으로 치지 않는다)",
       ht._payload_declared_missing(None) == frozenset())
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        ck("★PAYLOAD 부재 → 빈 집합", ht._payload_declared_missing(d) == frozenset())
        (d / "PAYLOAD.json").write_text(json.dumps({"missing": ["HINT_MISSING_LITE"]}), encoding="utf-8")
        ck("선언을 읽는다", ht._payload_declared_missing(d) == frozenset({"HINT_MISSING_LITE"}))
        (d / "PAYLOAD.json").write_text("{ not json", encoding="utf-8")
        ck("★파손 → 빈 집합(게이트가 스스로 열리지 않는다)", ht._payload_declared_missing(d) == frozenset())

    # A 는 여전히 무조건 차단이다 — 서빙 실패를 성공으로 위장한 배포가 계약 §2 의 유일한 위협이다.
    raisedA = None
    try:
        ht._require_serving_evidence("selftest", {"runtime": {"health_ok": False,
                                                              "functional_smoke_passed": True},
                                                  "evidence": {}}, None)
    except SystemExit as exc:
        raisedA = exc
    ck("★서빙 성공 허위(health_ok=false)는 무조건 차단", raisedA is not None and raisedA.code != 0)

    print("── ④ 여정 정보는 여전히 필수다(fail-closed) ──")
    # 여정 = 무엇을 시도했고 어디서 멈췄나. 저작 마커가 남아 있으면 그 정보가 **없는** 것이다.
    ck("저작 마커 상수 존재", bool(hc.AGENT_MARK))
    ck("★미저작 마커가 본문에 남으면 그것이 곧 여정 부재 신호", hc.AGENT_MARK in filled)

    print("[selftest_hint_gate] " + ("PASS" if not FAILURES else f"FAIL ({len(FAILURES)})"))
    for f in FAILURES:
        print(f"  - {f}", file=sys.stderr)
    return 0 if not FAILURES else 1


if __name__ == "__main__":
    sys.exit(main())
