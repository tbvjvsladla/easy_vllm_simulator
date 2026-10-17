#!/usr/bin/env python3
"""selftest_campaign_lifecycle.py — 캠페인 수명 상태(보류·재개·포기·선점)의 자체검사.

plan_26101719 의 합격 묶음 네 개를 픽스처 저장소에서 결정론으로 시험한다.
  state      — journey fold(옛 인스턴스 기본값 · orphan · unknown)
  transition — 단일 정문(허점 봉쇄 · 협조 거부 · --preempt · teardown 실패 fail-closed · 토폴로지 2단)
  requery    — 재질의 계기 (a)(d)만 · 답한 전이는 다시 묻지 않음 · 리볼빙 N회 무질의 · 첫 줄 상태
  guard      — 진행성 쓰기 거부 · 정리성 허용 · 보류 purge 불가 · abandoned 축소 게이트 · 재개 게이트
  propagate  — 메인 종결의 서브 전파(plan_26101722 K1~K3): 종결 줄의 sub_nodes · 미수렴 첫 줄 · purge 조건 · 사람 수용
  assign     — 배정 writer(K8): pending 셀만 · 4필드 · 이중 배정·hint 승인 셀 거부 · reslice 미수렴·echo · 서브 slice-update

라이브(실제 서브 A2A · 실제 teardown · 실제 브랜치 전환)는 이 파일의 범위가 아니다 — E2E testlog 가 맡는다.
픽스처가 실물보다 좁을 수 있다는 것을 알고 쓴다(teardown·docker·git 은 주입점으로 대체한다).

사용: python3 selftest_campaign_lifecycle.py [-k state|transition|requery|guard]
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("_ci_lifecycle_test", HERE / "campaign_init.py")
ci = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ci)

UTC = "2026-10-17T11:00:00Z"
OK = True


def ck(label, cond):
    global OK
    print(("  PASS " if cond else "  FAIL ") + label)
    OK = OK and bool(cond)


def run(*argv) -> "tuple[int, str]":
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = ci.main(list(argv))
    return rc, out.getvalue() + err.getvalue()


class Fixture:
    def __init__(self, tmp: Path):
        self.tmp = tmp
        self.topology = "single"
        self.images = {}
        self.calls = []
        self.release_rc = 0
        ci.REPO_ROOT = tmp
        ci.CAMPAIGNS = tmp / "campaigns"
        ci.ACTIVE_POINTER = ci.CAMPAIGNS / "ACTIVE"
        ci.CAMPAIGNS.mkdir(parents=True)
        (tmp / "docs" / "testlog").mkdir(parents=True)
        (tmp / "docs" / "devlog").mkdir(parents=True)
        ci._CHECKOUT_TOPOLOGY_FN = lambda: self.topology
        ci._IMAGE_DIGEST_FN = lambda tag: self.images.get(tag)
        ci._RELEASE_RUNNER = self._runner

    def _runner(self, cmd):
        self.calls.append(cmd)
        return self.release_rc, "fixture teardown"

    def camp(self, cid, *, topology="single", active=False, opened=True, sub=False):
        base = ci.CAMPAIGNS / cid
        for d in ("cells", "phases", "sweeps", "relay"):
            (base / d).mkdir(parents=True, exist_ok=True)
        nodes = [{"node_id": "main", "role": "main", "topology": topology}]
        if sub:
            nodes.append({"node_id": "sub1", "role": "sub", "topology": topology})
        (base / "campaign.yaml").write_text(json.dumps({
            "schema_version": 1, "id": cid, "plan_ref": "docs/plan/p.md", "nodes": nodes,
            "assignments": {"main": [{"cell": "c1"}]},
            "control_variables": {"model": "m", "vllm_version": "0.31.0", "topology": topology}}),
            encoding="utf-8")
        (base / "evidence_pointers.json").write_text(json.dumps({"campaign_id": cid, "pointers": []}),
                                                    encoding="utf-8")
        if opened:
            ci.writer_append_journey(base, ci._lifecycle_event("campaign_opened", UTC, topology=topology,
                                                               approved_by="fixture"))
        if active:
            ci.ACTIVE_POINTER.write_text(cid + "\n", encoding="utf-8")
        return base

    def running(self, base, cell="c1"):
        ci.writer_set_phase(base, node="main", phase="bench", state="running", predicate="측정 중",
                            ok=False, source="fixture", cell_id=cell, started_utc=UTC, ended_utc=None)

    def testlog(self, name="testlog_26101711_포기요약.md"):
        p = self.tmp / "docs" / "testlog" / name
        p.write_text("# 포기 요약\n", encoding="utf-8")
        return f"docs/testlog/{name}"

    def journey_len(self, cid):
        return len(ci.read_journey(ci.CAMPAIGNS / cid))

    def state(self, cid):
        return ci.fold_lifecycle(ci.CAMPAIGNS / cid)["state"]


def g_state(fx: Fixture):
    print("## state — journey fold")
    legacy = fx.camp("L-closed", opened=False)
    (legacy / ci.JOURNEY_NAME).write_text(json.dumps({"utc": UTC, "kind": "campaign_closed"}) + "\n",
                                          encoding="utf-8")
    ck("옛 campaign_closed 줄 → closed(completed) · 이주 이벤트 없음",
       ci.fold_lifecycle(legacy)["state"] == "closed" and ci.fold_lifecycle(legacy)["close_reason"] == "completed")
    fx.camp("L-active", opened=False, active=True)
    ck("이벤트 없음 + ACTIVE 지목 → active", fx.state("L-active") == "active")
    ci.ACTIVE_POINTER.write_text("_bootstrap\n", encoding="utf-8")
    ck("이벤트 없음 + ACTIVE 미지목 → orphan", fx.state("L-active") == "orphan")
    ck("orphan 은 purge 게이트가 abandon 으로 안내한다",
       any("--abandon" in r for r in ci.purge_gate_reasons("L-active")))
    u = fx.camp("U1")
    ci.writer_append_journey(u, {"utc": UTC, "kind": "campaign_teleported", "lifecycle_v": 1})
    ck("모르는 campaign_* 이벤트 → unknown", fx.state("U1") == "unknown")
    u2 = fx.camp("U2")
    ci.writer_append_journey(u2, {"utc": UTC, "kind": "campaign_suspended", "lifecycle_v": 99})
    ck("미래 lifecycle_v → unknown(버전 차 · D15)", fx.state("U2") == "unknown")
    rc, msg = run("--campaign-id", "U1", "--evidence-add", "--kind", "testlog", "--path", fx.testlog())
    ck("unknown 캠페인은 정리성 쓰기까지 전부 거부(rc≠0 · 동기화 안내)", rc != 0 and "동기화" in msg)
    ck("첫 줄이 unknown 을 정리 대상으로 보인다", "U1=unknown" in ci.lifecycle_line())
    # 서브 렌더 트리 폴백(2026-10-17 라이브): 4자일치 술어가 없는 노드는 manifest 로 토폴로지를 읽는다.
    saved_fn, saved_mod = ci._CHECKOUT_TOPOLOGY_FN, ci._topology_parity_module
    try:
        ci._CHECKOUT_TOPOLOGY_FN = None
        ci._topology_parity_module = lambda: (_ for _ in ()).throw(ci.TransitionRefusal("술어 부재(서브 렌더 트리)"))
        ck("술어 없음 + manifest 없음 → 모른다(None · 추측 ✗)", ci.checkout_topology() is None)
        (fx.tmp / "output" / "single").mkdir(parents=True)
        (fx.tmp / "output" / "single" / "manifest.yaml").write_text("topology: single  # 실측\n", encoding="utf-8")
        ck("술어 없음 → manifest(output/single) 의 topology 를 읽는다", ci.checkout_topology() == "single")
        (fx.tmp / "output" / "multi").mkdir(parents=True)
        (fx.tmp / "output" / "multi" / "manifest.yaml").write_text("topology: multi\n", encoding="utf-8")
        ck("manifest 가 둘이면 모른다(None)", ci.checkout_topology() is None)
    finally:
        ci._CHECKOUT_TOPOLOGY_FN, ci._topology_parity_module = saved_fn, saved_mod


def g_transition(fx: Fixture):
    print("## transition — 단일 정문")
    a = fx.camp("A", active=True)
    rc, msg = run("--init", "B", "--plan-ref", "docs/plan/p.md", "--apply")
    ck("허점 봉쇄: 처분 없는 --init 은 rc≠0 · ACTIVE 불변 · B 미생성",
       rc != 0 and "처분 없이" in msg and ci.active_campaign_id() == "A" and not (ci.CAMPAIGNS / "B").exists())
    ck("허점 안내문이 정상 차단 분류와 세 경로(보류·close·abandon)를 말한다",
       "정상 차단" in msg and "--suspend-current" in msg and "--close" in msg and "--abandon" in msg)
    fx.running(a)
    n0 = fx.journey_len("A")
    rc, msg = run("--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A",
                  "--release", "cfgA", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("협조 거부: running 셀이 있으면 rc≠0 · '--preempt' 안내 · 아무것도 쓰지 않음",
       rc != 0 and "--preempt" in msg and ci.active_campaign_id() == "A" and fx.journey_len("A") == n0
       and not fx.calls)
    fx.release_rc = 2
    rc, msg = run("--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A",
                  "--preempt", "--release", "cfgA", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("teardown 실패 → fail-closed: rc≠0 · ACTIVE 불변 · journey 불변",
       rc != 0 and "teardown 실패" in msg and ci.active_campaign_id() == "A" and fx.journey_len("A") == n0)
    fx.release_rc = 0
    rc, msg = run("--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A",
                  "--approved-by", "fixture", "--utc", UTC, "--preempt", "--apply")
    ck("--release 누락은 거부(침묵 skip ✗)", rc != 0 and "--release" in msg and ci.active_campaign_id() == "A")
    fx.calls.clear()
    rc, msg = run("--init", "B", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "A",
                  "--preempt", "--release", "cfgA", "--approved-by", "fixture", "--utc", UTC, "--apply")
    fa = ci.fold_lifecycle(ci.CAMPAIGNS / "A")
    fb = ci.fold_lifecycle(ci.CAMPAIGNS / "B")
    ck("--preempt 원자 전이: rc 0 · ACTIVE=B · A suspended · B active(parent=A)",
       rc == 0 and ci.active_campaign_id() == "B" and fa["state"] == "suspended"
       and fb["state"] == "active" and fb["parent"] == "A")
    ck("정식 teardown 진입점을 config 로 불렀다(single_serve_down.sh cfgA)",
       len(fx.calls) == 1 and fx.calls[0][1].endswith("single_serve_down.sh") and fx.calls[0][2] == "cfgA")
    ck("중단 셀 c1 이 interrupted 로 남고 phase 는 failed(preempted)",
       fa["interrupted_cells"] == ["c1"]
       and ci._read_json(a / "phases" / "main" / "bench.status.json")["state"] == "failed")
    ck("PARKED devlog 1건이 docs 평면에 났다",
       fa["parked_devlog"] and (fx.tmp / fa["parked_devlog"]).is_file()
       and "PARKED:" in (fx.tmp / fa["parked_devlog"]).read_text(encoding="utf-8"))
    ck("B 선언에 topology 가 대상값으로 적혔다",
       ci.read_declaration(ci.CAMPAIGNS / "B")["control_variables"]["topology"] == "single")

    print("## transition — 토폴로지 교차 2단")
    x = fx.camp("X", active=False)
    ci.ACTIVE_POINTER.write_text("_bootstrap\n", encoding="utf-8")
    # B 를 정리해 ACTIVE 를 비우고 X 를 활성으로 둔다.
    ci.writer_append_journey(ci.CAMPAIGNS / "B", ci._lifecycle_event("campaign_closed", UTC, reason="completed"))
    ci.ACTIVE_POINTER.write_text("X\n", encoding="utf-8")
    rc, msg = run("--init", "M", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "X",
                  "--target-topology", "multi", "--release", "none", "--approved-by", "fixture", "--utc", UTC,
                  "--dispose", "A=keep", "--apply")
    ck("1단: X 보류 · ACTIVE=_bootstrap · M 은 아직 없다 · 개설 대기 표시",
       rc == 0 and fx.state("X") == "suspended" and ci.active_campaign_id() == ci.BOOTSTRAP
       and not (ci.CAMPAIGNS / "M").exists() and "개설 대기 M@multi" in ci.lifecycle_line())
    rc, msg = run("--init", "M", "--plan-ref", "docs/plan/p.md", "--utc", UTC, "--apply")
    ck("반대 브랜치(single)에서 2단 시도 → 거부(4자일치 · 오배달 방지)",
       rc != 0 and "대상 브랜치" in msg and not (ci.CAMPAIGNS / "M").exists())
    fx.topology = "multi"
    rc, msg = run("--init", "M", "--plan-ref", "docs/plan/p.md", "--utc", UTC, "--apply")
    fm = ci.fold_lifecycle(ci.CAMPAIGNS / "M")
    ck("2단(multi 체크아웃): M active · parent=X · topology=multi · 개설 대기 해소",
       rc == 0 and fm["state"] == "active" and fm["parent"] == "X"
       and ci.read_declaration(ci.CAMPAIGNS / "M")["control_variables"]["topology"] == "multi"
       and "개설 대기" not in ci.lifecycle_line())
    rc, msg = run("--resume-campaign", "X", "--approved-by", "fixture", "--utc", UTC, "--accept-drift", "--apply")
    ck("multi 에서 single 캠페인 재개 → 무조건 거부(drift 아님 · --accept-drift 무력)",
       rc != 0 and "drift 가 아니라" in msg and fx.state("X") == "suspended")
    rc, msg = run("--abandon", "M", "--reason", "fixture", "--summary-testlog", fx.testlog("testlog_M.md"),
                  "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("M 포기 → X 에 재개 미결(계기 a · abandoned 도 1회)",
       rc == 0 and fx.state("X") == "resume_pending" and "X←M 종결(abandoned): 재개?" in ci.lifecycle_line())
    fx.topology = "single"
    rc, msg = run("--resume-campaign", "X", "--approved-by", "fixture", "--utc", UTC, "--apply")
    fx_ = ci.fold_lifecycle(ci.CAMPAIGNS / "X")
    ck("single 복귀 후 X 재개 · 인계 포인터에 M 종결 사유",
       rc == 0 and ci.active_campaign_id() == "X" and fx_["state"] == "active"
       and fx_["handoff_pointers"] and fx_["handoff_pointers"][0]["child"] == "M"
       and fx_["handoff_pointers"][0]["child_reason"] == "abandoned")


def g_requery(fx: Fixture):
    print("## requery — 계기 (a)(d)만")
    a = fx.camp("RA", active=True)
    run("--init", "RB", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "RA",
        "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    n0 = fx.journey_len("RA")
    first = [ci.resume_brief().splitlines()[0] for _ in range(5)]
    ck("리볼빙 5회: 첫 줄 동일 · 미결 0 · journey 불변(재질의 0회)",
       len(set(first)) == 1 and "미결 질의 0" in first[0] and fx.journey_len("RA") == n0)
    ck("첫 줄 위치 고정: resume-brief 1행이 '상태:' 줄", ci.resume_brief().splitlines()[0].startswith("상태: "))
    rc, msg = run("--abandon", "RB", "--reason", "탐색 종료", "--summary-testlog", fx.testlog("testlog_RB.md"),
                  "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("자식 종결 → 부모 미결 정확히 1건", rc == 0 and ci.lifecycle_line().count("RA←RB") == 1
       and "미결 질의 1" in ci.lifecycle_line())
    first = [ci.resume_brief().splitlines()[0] for _ in range(3)]
    ck("무인 리볼빙: 미결은 줄로만 보이고 쓰기가 늘지 않는다", len(set(first)) == 1
       and fx.journey_len("RA") == n0 + 1)
    rc, msg = run("--answer-query", "RA", "--query-decision", "keep", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("보류 유지 답 → suspended · 답함 · 미결 0", rc == 0 and fx.state("RA") == "suspended"
       and "미결 질의 0" in ci.lifecycle_line())
    rc, msg = run("--answer-query", "RA", "--query-decision", "keep", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("같은 답 재입력은 멱등 no-op", rc == 0 and "no-op" in msg)
    rc, msg = run("--init", "RC", "--plan-ref", "docs/plan/p.md", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("무관 개설(계기 d): 이미 답한 RA 는 다시 묻지 않는다 → 처분 없이 개설", rc == 0 and fx.state("RC") == "active")
    rc, msg = run("--init", "RD", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none",
                  "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("지금 보류되는 RC 는 이 전이 자체가 처분이라 묻지 않는다", rc == 0 and fx.state("RC") == "suspended")
    rc, msg = run("--init", "RE", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none",
                  "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("답 이후 새로 보류된 RC 만 묻는다(RA 제외) · 처분 없으면 거부",
       rc != 0 and "RC" in msg.split("처분이 필요하다:")[-1] and "RA" not in msg.split("처분이 필요하다:")[-1]
       and fx.state("RD") == "active")
    rc, msg = run("--init", "RE", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--release", "none",
                  "--dispose", "RC=keep", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("--dispose RC=keep 로 전사하면 개설 · RC 답함",
       rc == 0 and fx.state("RE") == "active" and ci.fold_lifecycle(ci.CAMPAIGNS / "RC")["answered"])
    rc, msg = run("--init", "RF", "--plan-ref", "docs/plan/p.md", "--suspend-current", "--derived-from", "RE",
                  "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("파생 개설은 계기 d 가 아니다(RD 미답이어도 묻지 않는다)", rc == 0 and fx.state("RF") == "active")


def g_guard(fx: Fixture):
    print("## guard — 쓰기·purge·재개 게이트")
    a = fx.camp("GA", active=True, sub=True)
    st = a / "cells" / "c1"
    st.mkdir(parents=True, exist_ok=True)
    ci._write_json(st / "cell.status.json", {"schema_version": 1, "cell_id": "c1", "cell_outcome": "measured",
                                             "image": {"tag": "img:1", "digest": "sha256:aaa"}})
    fx.images["img:1"] = "sha256:aaa"
    rc, msg = run("--suspend", "GA", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    fa = ci.fold_lifecycle(a)
    ck("단독 보류 · 서브 대상이 있으면 '서브 미수렴'", rc == 0 and fa["state"] == "suspended"
       and fa["sub_convergence"] == "unconverged" and "서브 미수렴 GA@sub1" in ci.lifecycle_line())
    rc, msg = run("--sub-echo", "GA", "--node", "sub1", "--control", "suspend", "--utc", UTC, "--apply")
    ck("서브 echo → converged", rc == 0 and ci.fold_lifecycle(a)["sub_convergence"] == "converged")
    for flag in (("--cell-set", "c1", "--outcome", "pending"),
                 ("--phase-set", "serve", "--node", "main", "--state", "running")):
        rc, msg = run("--campaign-id", "GA", *flag)
        ck(f"보류 캠페인 진행성 쓰기 거부 {flag[0]}", rc != 0 and "진행성 쓰기" in msg)
    rc, msg = run("--campaign-id", "GA", "--evidence-add", "--kind", "testlog", "--node", "main",
                  "--path", fx.testlog("testlog_GA.md"))
    ck("보류 캠페인 정리성 쓰기(--evidence-add) 허용", rc == 0)
    ck("보류 상태 purge 불가", any("보류 상태" in r for r in ci.purge_gate_reasons("GA")))
    rc, msg = run("--abandon", "GA", "--reason", "x", "--summary-testlog", fx.testlog("testlog_GA2.md"),
                  "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("abandoned 축소 게이트: 측정 셀 c1 증거 포인터 없음 → 거부", rc != 0 and "c1" in msg)
    fx.images["img:1"] = "sha256:bbb"
    rc, msg = run("--resume-campaign", "GA", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("재개 게이트: digest drift → 거부 · --accept-drift 안내", rc != 0 and "--accept-drift" in msg
       and fx.state("GA") == "suspended")
    rc, msg = run("--resume-campaign", "GA", "--approved-by", "fixture", "--utc", UTC, "--accept-drift", "--apply")
    ck("--accept-drift 로 재개 · 서브 재개는 A2A 안내", rc == 0 and fx.state("GA") == "active" and "campaign-control" in msg)
    # interrupted → pending
    b = fx.camp("GB")
    ci.ACTIVE_POINTER.write_text("GB\n", encoding="utf-8")
    ci.writer_set_cell(b, cell="c1", outcome="measured", node=None, version=None, model=None, decode_tps="10",
                       measurement_source="fixture", void_reason=None, void_reason_source=None,
                       axis_citation=None, next_intent=None, utc=UTC)
    fx.running(b)
    run("--suspend", "GB", "--preempt", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ci.ACTIVE_POINTER.write_text("_bootstrap\n", encoding="utf-8")
    rc, msg = run("--resume-campaign", "GB", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("재개 시 interrupted 셀은 pending 으로 돌아간다",
       rc == 0 and ci._read_json(b / "cells" / "c1" / "cell.status.json")["cell_outcome"] == "pending"
       and ci.fold_lifecycle(b)["interrupted_cells"] == [])
    rc, msg = run("--resume-campaign", "GB", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("활성 캠페인 재개 재시도는 거부(이미 active)", rc != 0)
    rc, msg = run("--resume-campaign", "GA", "--utc", UTC, "--apply")
    ck("--approved-by 없는 전이는 거부(승인 없는 전이 ✗)", rc != 0 and "--approved-by" in msg)
    # 2026-10-17 라이브 E2: 서브 자기 인스턴스(self_role=sub)는 자기 자신을 echo 대상으로 세지 않는다.
    sb = fx.camp("GS", active=True, sub=True)
    d = ci.read_declaration(sb)
    d["self_role"] = "sub"
    d["nodes"] = [n for n in d["nodes"] if n["role"] == "sub"]
    (sb / "campaign.yaml").write_text(json.dumps(d), encoding="utf-8")
    run("--suspend", "GS", "--release", "none", "--approved-by", "main-a2a", "--utc", UTC, "--apply")
    # 2026-10-17 라이브 E7: 2노드 캠페인의 포기는 요약 증거를 main 노드로 귀속한다.
    two = fx.camp("G2", active=True, sub=True)
    rc, msg = run("--abandon", "G2", "--reason", "x", "--summary-testlog", fx.testlog("testlog_G2.md"),
                  "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    ck("2노드 캠페인 포기: 요약 증거가 main 노드로 귀속되고 종결된다",
       rc == 0 and fx.state("G2") == "closed"
       and any(p.get("node_id") == "main" for p in ci._read_json(two / "evidence_pointers.json")["pointers"]))
    ck("서브 자기 인스턴스 보류는 '서브 미수렴' 을 만들지 않는다",
       ci.fold_lifecycle(sb)["sub_convergence"] is None and "GS@" not in ci.lifecycle_line())


def g_relay(fx: Fixture):
    print("## guard(relay) — 감독 전진 판정 보류 · A2A 제어 지시 · echo 판정")
    import types
    _rs = importlib.util.spec_from_file_location("_relay_lifecycle_test", HERE / "relay.py")
    relay = importlib.util.module_from_spec(_rs)
    _rs.loader.exec_module(relay)
    t = relay.campaign_control_task("SA", "suspend", preempt=False)
    ck("보류 지시 본문: --suspend · --release · echo 형식 · 개설(--init) 지시 없음",
       "--suspend SA" in t and "--release" in t and "campaign_control_echo" in t and "--init" not in t)
    ck("재개 지시 본문: --resume-campaign · drift 우회 금지", "--resume-campaign SA" in
       relay.campaign_control_task("SA", "resume") and "우회하지 말고" in relay.campaign_control_task("SA", "resume"))
    ok, _ = relay.campaign_control_echo_ok({"campaign_control_echo": {"campaign_id": "SA", "control": "suspend",
                                                                       "state": "suspended"}}, "SA", "suspend")
    bad, why = relay.campaign_control_echo_ok({"campaign_control_echo": {"campaign_id": "SA", "control": "suspend",
                                                                          "state": "active"}}, "SA", "suspend")
    none, _ = relay.campaign_control_echo_ok({}, "SA", "suspend")
    ck("echo 판정: 일치=수렴 · 상태 불일치·부재=미수렴", ok and not bad and "active" in why and not none)
    relay._RELAY_CAMPAIGN = "SA"
    ck("수명 제어 원장은 대상 캠페인 relay 아래(ACTIVE=_bootstrap 이어도 대기실 ✗)",
       relay.relay_root(str(fx.tmp)).endswith("campaigns/SA/relay"))
    relay._RELAY_CAMPAIGN = None
    base = fx.camp("SA", active=True)
    led = base / "relay" / "ctx-sa.json"
    led.write_text(json.dumps({"context_id": "ctx-sa", "campaign_node": "main", "attempts": [{"x": 1}]}),
                   encoding="utf-8")
    relay.supervise_decide = lambda doc, **kw: {"action": "next_cell", "reason": "fixture"}
    args = types.SimpleNamespace(supervise_step="SA", repo_root=str(fx.tmp), utc=UTC, runners=None,
                                 backend=None, model=None, cost_cap_attempts=None, apply=False, ladder=None)
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        relay.supervise_step(args)
    ck("active 캠페인: 감독 전진 판정 그대로(next_cell)", "next_cell" in out.getvalue())
    run("--suspend", "SA", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        relay.supervise_step(args)
    steps = json.loads(led.read_text(encoding="utf-8")).get("supervisor_steps") or []
    ck("보류 캠페인: 감독이 전진 판정을 hold 로 바꾸고 원장에 적는다",
       "hold" in out.getvalue() and steps and steps[-1]["action"] == "hold")
    relay.supervise_decide = lambda doc, **kw: {"action": "popup", "reason": "처음 보는 종료(fixture)"}
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        rc3 = relay.supervise_step(args)
    ck("보류 캠페인: popup 도 hold 로 — 리볼빙마다 같은 것을 묻지 않는다(라이브 E4)",
       "→ popup" not in out.getvalue() and "→ hold" in out.getvalue() and rc3 == 0)


def g_propagate(fx: Fixture):
    print("## propagate — 메인 종결의 서브 전파(K1~K3)")
    a = fx.camp("PA", active=True, sub=True)
    rc, msg = run("--abandon", "PA", "--reason", "x", "--summary-testlog", fx.testlog("testlog_PA.md"),
                  "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    f = ci.fold_lifecycle(a)
    ck("2노드 포기: 종결 줄이 서브를 지목하고 미수렴으로 접힌다 · relay abandon 안내",
       rc == 0 and f["sub_nodes"] == ["sub1"] and f["sub_convergence"] == "unconverged"
       and "--campaign-control abandon" in msg)
    ck("첫 줄에 '서브 미수렴 PA@sub1'", "서브 미수렴 PA@sub1" in ci.lifecycle_line())
    ck("purge 게이트: 서브 종결 미수렴이면 닫히고 relay·수용 두 경로를 말한다",
       any("서브 종결이 수렴하지 않았다" in r and "--sub-unconverged-accept" in r for r in ci.purge_gate_reasons("PA")))
    run("--sub-echo", "PA", "--node", "sub1", "--control", "close", "--utc", UTC, "--apply")
    ck("★음성대조 사유가 다른 echo(close)는 포기 종결을 수렴시키지 않는다",
       ci.fold_lifecycle(a)["sub_convergence"] == "unconverged")
    rc, _ = run("--sub-echo", "PA", "--node", "sub1", "--control", "abandon", "--utc", UTC, "--apply")
    ck("abandon echo → 수렴 · purge 게이트의 서브 사유 소거",
       rc == 0 and ci.fold_lifecycle(a)["sub_convergence"] == "converged"
       and not any("서브 종결" in r for r in ci.purge_gate_reasons("PA")) and "PA@" not in ci.lifecycle_line())
    b = fx.camp("PB", active=True, sub=True)
    run("--abandon", "PB", "--reason", "x", "--summary-testlog", fx.testlog("testlog_PB.md"),
        "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    rc, msg = run("--sub-unconverged-accept", "PB", "--node", "sub1", "--utc", UTC, "--apply")
    ck("★음성대조 수용은 사람 발화 전사 없이는 거부", rc != 0 and "--approved-by" in msg)
    rc, _ = run("--sub-unconverged-accept", "PB", "--node", "sub1", "--approved-by", "사람: 서브 미도달 수용",
                "--utc", UTC, "--apply")
    fb = ci.fold_lifecycle(b)
    ck("사람 수용 → 수렴 · 수용 기록이 따로 남는다 · purge 서브 사유 소거",
       rc == 0 and fb["sub_convergence"] == "converged" and fb["sub_accepted"] == ["sub1"]
       and not any("서브 종결" in r for r in ci.purge_gate_reasons("PB")))
    fx.camp("PC", active=True, sub=True)
    rc, msg = run("--sub-unconverged-accept", "PC", "--node", "sub1", "--approved-by", "x", "--utc", UTC, "--apply")
    ck("★음성대조 active 캠페인에는 수용할 것이 없다(보류·재개 미수렴은 차단이 아니다)", rc != 0)
    ci.ACTIVE_POINTER.write_text("_bootstrap\n", encoding="utf-8")
    old = fx.camp("PD", opened=False, sub=True)
    (old / ci.JOURNEY_NAME).write_text(json.dumps({"utc": UTC, "kind": "campaign_closed", "reason": "abandoned",
                                                    "lifecycle_v": 1, "summary_testlog": fx.testlog("t_PD.md")})
                                       + "\n", encoding="utf-8")
    ck("옛 종결(sub_nodes 없음)은 전파 조건 면제 — 소급 차단 ✗",
       ci.fold_lifecycle(old)["sub_convergence"] is None
       and not any("서브 종결" in r for r in ci.purge_gate_reasons("PD")))
    _rs = importlib.util.spec_from_file_location("_relay_prop_test", HERE / "relay.py")
    relay = importlib.util.module_from_spec(_rs)
    _rs.loader.exec_module(relay)
    tc = relay.campaign_control_task("PA", "close")
    ta = relay.campaign_control_task("PA", "abandon", reason="모드3 탐색으로 전환")
    ck("relay 지시: close=--close · abandon=--abandon+사유+요약 testlog · echo 에 close_reason",
       "--close PA" in tc and "--abandon PA" in ta and "모드3 탐색으로 전환" in ta and "--summary-testlog" in ta
       and "close_reason" in ta and "--init" not in ta)
    ok, _ = relay.campaign_control_echo_ok({"campaign_control_echo": {"campaign_id": "PA", "control": "abandon",
                                                                       "state": "closed", "close_reason": "abandoned"}},
                                           "PA", "abandon")
    bad, why = relay.campaign_control_echo_ok({"campaign_control_echo": {"campaign_id": "PA", "control": "abandon",
                                                                          "state": "closed",
                                                                          "close_reason": "completed"}}, "PA", "abandon")
    ck("relay echo 판정: 같은 closed 라도 사유가 다르면 미수렴", ok and not bad and "사유" in why)
    ts = relay.orphan_sweep_task(["o1", "o2"], "옛 허점 잔재")
    ck("orphan sweep 지시: 대상 전수 --abandon · 요약 testlog 1건 · 목록 밖 금지",
       "--abandon o1" in ts and "--abandon o2" in ts and "목록 밖" in ts and "--init" not in ts)
    _r = lambda res: {"campaign_control_echo": {"control": "orphan_sweep", "summary_testlog": "t.md", "results": res}}
    ok, _ = relay.orphan_sweep_echo_ok(_r([{"campaign_id": c, "state": "closed", "close_reason": "abandoned"}
                                           for c in ("o1", "o2")]), ["o1", "o2"])
    part, why = relay.orphan_sweep_echo_ok(_r([{"campaign_id": "o1", "state": "closed", "close_reason": "abandoned"},
                                               {"campaign_id": "o2", "state": "orphan", "refusal": "게이트"}]),
                                           ["o1", "o2"])
    extra, why2 = relay.orphan_sweep_echo_ok(_r([{"campaign_id": c, "state": "closed", "close_reason": "abandoned"}
                                                 for c in ("o1", "o2", "o3")]), ["o1", "o2"])
    ck("★orphan sweep echo: 전수 abandoned=수렴 · 일부 거부=미정리(사유) · 목록 밖 접촉=거부",
       ok and not part and "o2" in why and "게이트" in why and not extra and "o3" in why2)


def g_assign(fx: Fixture):
    print("## assign — 배정 writer · reslice(K8)")
    base = fx.camp("AS", active=True, sub=True)
    d = ci.read_declaration(base)
    d["assignments"] = {"main": [{"cell": "c1"}, {"cell": "c2"}], "sub1": [{"cell": "s1"}]}
    d["hint_targets"] = [{"node_id": "main", "cells": ["c2"], "approval": {"approved_by": "사람", "utc": UTC}}]
    (base / "campaign.yaml").write_text(json.dumps(d), encoding="utf-8")
    four = ("--assign-reason", "모드3 조건 추가", "--assign-evidence", "docs/plan/p.md",
            "--assign-approved-by", "사람: 승인", "--utc", UTC)
    rc, msg = run("--assign-set", "main", "--assign-cells", "c2,c1:HITL,c3", "--assign-reason", "x", "--utc", UTC)
    ck("★음성대조 4필드 누락 거부", rc != 0 and "필수" in msg)
    rc, msg = run("--assign-set", "main", "--assign-cells", "c2,c1:HITL,c3", *four)
    d2 = ci.read_declaration(base)
    ck("pending 셀 추가·순서·mode 변경 → assignments 갱신 · revisions[0]=t0 · journey 1줄",
       rc == 0 and [i["cell"] for i in d2["assignments"]["main"]] == ["c2", "c1", "c3"]
       and d2["assignment_revisions"][0]["approved_by"] == "t0" and d2["assignment_revisions"][1]["added"] == ["c3"]
       and any(e.get("kind") == "campaign_assignments_revised" for e in ci.read_journey(base))
       and ci.fold_lifecycle(base)["state"] == "active")
    ci.writer_set_phase(base, node="main", phase="bench", state="done", predicate="x", ok=True, source="fixture",
                        cell_id="c3", started_utc=UTC, ended_utc=UTC)
    rc, msg = run("--assign-set", "main", "--assign-cells", "c2,c1:HITL", *four)
    ck("★음성대조 done 셀 제거 거부(사유에 phase 지목)", rc != 0 and "c3" in msg and "pending" in msg)
    rc, msg = run("--assign-set", "main", "--assign-cells", "c1:HITL,c3", *four)
    ck("★음성대조 hint 승인 셀 제거 거부", rc != 0 and "hint" in msg)
    rc, msg = run("--assign-set", "main", "--assign-cells", "c2,c1:HITL,c3,s1", *four)
    ck("★음성대조 다른 노드 셀을 넣으면 거부(이동 = 제거+추가)", rc != 0 and "sub1" in msg)
    rc, msg = run("--assign-set", "sub1", "--assign-cells", "s1,s2", *four)
    f = ci.fold_lifecycle(base)
    ck("서브 몫 재배정 → 재배정 미수렴 · 첫 줄 · relay reslice 안내",
       rc == 0 and "sub1" in f["reslice_pending"] and "재배정 미수렴 AS@sub1" in ci.lifecycle_line()
       and "--campaign-control reslice" in msg)
    sl = ci.emit_slice(base, "sub1", utc=UTC)
    ck("파생 선언이 재배정 메타를 싣는다", (sl.get("_assignment_revision") or {}).get("rev") == f["reslice_pending"]["sub1"])
    rc, _ = run("--sub-echo", "AS", "--node", "sub1", "--control", "reslice", "--utc", UTC, "--apply")
    ck("★음성대조 rev 없는 reslice echo 거부", rc != 0)
    ci.record_sub_echo("AS", node="sub1", control="reslice", utc=UTC, apply=True, rev=f["reslice_pending"]["sub1"])
    ck("reslice echo(rev 일치) → 재배정 미수렴 소거", not ci.fold_lifecycle(base)["reslice_pending"]
       and "AS@" not in ci.lifecycle_line())
    _rs = importlib.util.spec_from_file_location("_relay_assign_test", HERE / "relay.py")
    relay = importlib.util.module_from_spec(_rs)
    _rs.loader.exec_module(relay)
    t = relay.campaign_control_task("AS", "reslice", slice_doc=sl)
    ok, _ = relay.campaign_control_echo_ok({"campaign_control_echo": {
        "campaign_id": "AS", "control": "reslice", "state": "active", "rev": sl["_assignment_revision"]["rev"],
        "assigned_cells": ["s1", "s2"]}}, "AS", "reslice", slice_doc=sl)
    bad, _ = relay.campaign_control_echo_ok({"campaign_control_echo": {
        "campaign_id": "AS", "control": "reslice", "state": "active", "rev": sl["_assignment_revision"]["rev"],
        "assigned_cells": ["s1"]}}, "AS", "reslice", slice_doc=sl)
    ck("relay reslice: 지시가 선언을 싣고 --slice-update - · echo 는 셀 목록·rev 까지 대조",
       "--slice-update -" in t and '"s2"' in t and ok and not bad)
    # 서브 쪽 — 다른 저장소의 자기 인스턴스(같은 id)를 흉내 낸다.
    sub_root = fx.tmp / "subrepo" / "campaigns"
    saved = (ci.CAMPAIGNS, ci.ACTIVE_POINTER)
    try:
        ci.CAMPAIGNS = sub_root
        ci.ACTIVE_POINTER = sub_root / "ACTIVE"
        sbase = sub_root / "AS"
        (sbase / "cells").mkdir(parents=True)
        own = {k: v for k, v in sl.items() if not k.startswith("_")}
        own["assignments"] = {"sub1": [{"cell": "s1"}]}
        (sbase / "campaign.yaml").write_text(json.dumps(own), encoding="utf-8")
        (sbase / "evidence_pointers.json").write_text(json.dumps({"pointers": []}), encoding="utf-8")
        ci.writer_append_journey(sbase, ci._lifecycle_event("campaign_opened", UTC, topology="single",
                                                            approved_by="main-a2a"))
        ci.ACTIVE_POINTER.write_text("AS\n", encoding="utf-8")
        sp = fx.tmp / "slice.json"
        sp.write_text(json.dumps(sl), encoding="utf-8")
        rc, msg = run("--slice-update", str(sp), "--utc", UTC, "--apply")
        ck("서브 --slice-update → 자기 배정이 메인 선언과 같아진다 · 승인 전사는 main-a2a",
           rc == 0 and [i["cell"] for i in ci.read_declaration(sbase)["assignments"]["sub1"]] == ["s1", "s2"]
           and ci.read_declaration(sbase)["assignment_revisions"][-1]["approved_by"].startswith("main-a2a"))
        ci.writer_append_journey(sbase, ci._lifecycle_event("campaign_suspended", UTC, approved_by="x"))
        rc, msg = run("--slice-update", str(sp), "--utc", UTC, "--apply")
        ck("★음성대조 보류 중 서브 재배정 거부(진행성 쓰기)", rc != 0 and "suspended" in msg)
    finally:
        ci.CAMPAIGNS, ci.ACTIVE_POINTER = saved
    rc, msg = run("--slice-update", str(fx.tmp / "slice.json"), "--utc", UTC, "--apply")
    ck("★음성대조 메인 인스턴스에 slice-update 거부(메인은 --assign-set)", rc != 0 and "서브 자기 인스턴스가 아니다" in msg)
    run("--suspend", "AS", "--release", "none", "--approved-by", "fixture", "--utc", UTC, "--apply")
    rc, msg = run("--assign-set", "main", "--assign-cells", "c2,c1:HITL,c3", "--campaign-id", "AS", *four)
    ck("★음성대조 보류 캠페인 재배정 거부", rc != 0)
    # K5: phase-set 이 --cell 만 받아도 배정에서 노드를 파생한다(lite·sweep 실행기 종료부).
    pb = fx.camp("PH", active=True)
    rc, _ = run("--phase-set", "bench", "--cell", "c1", "--state", "running", "--started-utc", UTC)
    st = ci._read_json(pb / "phases" / "main" / "bench.status.json") or {}
    ck("phase-set --cell(노드 생략) → 배정 노드로 기록", rc == 0 and st.get("state") == "running"
       and st.get("cell_id") == "c1")
    rc, msg = run("--phase-set", "bench", "--cell", "zz", "--state", "running")
    ck("★음성대조 배정 밖 셀은 노드를 파생하지 못해 거부", rc != 0 and "assignments" in msg)


GROUPS = {"state": g_state, "transition": g_transition, "requery": g_requery, "guard": g_guard,
          "guard_relay": g_relay, "propagate": g_propagate, "assign": g_assign}


def main(argv=None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    pick = argv[argv.index("-k") + 1] if "-k" in argv else None
    saved = (ci.REPO_ROOT, ci.CAMPAIGNS, ci.ACTIVE_POINTER)
    try:
        for name, fn in GROUPS.items():
            if pick and pick not in name:
                continue
            with tempfile.TemporaryDirectory() as tmp:
                fn(Fixture(Path(tmp)))
    finally:
        ci.REPO_ROOT, ci.CAMPAIGNS, ci.ACTIVE_POINTER = saved
        ci._CHECKOUT_TOPOLOGY_FN = ci._IMAGE_DIGEST_FN = ci._RELEASE_RUNNER = None
    print("[selftest_campaign_lifecycle] " + ("PASS" if OK else "FAIL"))
    return 0 if OK else 1


if __name__ == "__main__":
    sys.exit(main())
