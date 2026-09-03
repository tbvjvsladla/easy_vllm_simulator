#!/usr/bin/env python3
"""relay.py — 메인↔서브 **턴제 릴레이**의 실행자(A2A 평면).

왜 턴제인가(2026-09-03 · P2 · plan_26090317 Q5):
    서브→메인 방향에는 별도 채널을 **만들지 않는다**. 폴링 inbox 는 헌법의 자동 폴링 금지와
    충돌하고, 역방향 ssh 는 서브에게 메인 접속 권한을 주는 것이며, 서브의 쓰기 허용 표면
    (`configs/`·`envs/`·`tasks/`·`output/**`) 밖이다. 서브가 메인에게 할 말은 **이미 있는 통로**
    — task-report — 로 온다(§2.7.8 도 그렇게 설계돼 있다). 그러므로 릴레이는:

        delegate → 리포트 수신 → (input-required 면) 답을 실어 **같은 세션 재개** → …

    이 파일이 그 왕복의 상태를 `tasks/<context_id>.json`(파일 = 세션)에 적고, 사람이 답해야 하는
    것만 `tasks/pending_hitl.json` 에 표면화한다.

규율(참고 프로젝트 차용 — memory: hermes-control-plane-reference-turn-budget):
    · scope ⊥ budget · 소진은 terminal → **더 큰 예산의 새 attempt**(예산 축소 금지)
    · SILENT_FALLBACK 금지 — 메인이 대신 한 것을 서브 성공으로 집계하지 않는다
    · 원장 4필드(task_grade·max_turns_allocated·max_turns_used·budget_outcome)를 매 턴 적는다
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
AGENT_CONTROL = os.path.join(REPO, ".claude", "policies", "runtime", "agent_control.py")
sys.path.insert(0, HERE)
import turn_budget  # noqa: E402
import bootstrap_canary as _canary  # noqa: E402  (manifest → target 해소를 재사용)

TASKS_DIR_NAME = "tasks"
PENDING_HITL = "pending_hitl.json"


def ledger_path(repo_root: str, context_id: str) -> str:
    return os.path.join(repo_root, TASKS_DIR_NAME, f"{context_id}.json")


def load_ledger(path: str) -> dict:
    if not os.path.exists(path):
        return {"schema_version": 1, "context_id": None, "attempts": []}
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def save_ledger(path: str, doc: dict) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    os.replace(tmp, path)


def latest_session_id(doc: dict):
    """이어 붙일 세션. **완결된 attempt 의 세션은 잇지 않는다** — 새 attempt 는 새 맥락이다."""
    for att in reversed(doc.get("attempts") or []):
        if att.get("budget_outcome") == "exhausted":
            return None                      # 소진된 세션을 이으면 그 자리에서 또 소진된다
        if att.get("status") == "input-required" and att.get("session_id"):
            return att["session_id"]
    return None


def next_budget(doc: dict, grade: str) -> dict:
    """직전 attempt 가 소진이면 증액한다. 그 외에는 grade 표의 기본값."""
    for att in reversed(doc.get("attempts") or []):
        if att.get("budget_outcome") == "exhausted" and att.get("max_turns_allocated"):
            return turn_budget.escalate(grade, int(att["max_turns_allocated"]))
        break
    return turn_budget.budget(grade)


def build_request(topology: str, manifest: str, task: str, grade: str,
                  resume_session_id=None, capabilities=None) -> dict:
    base = _canary.build_request(topology, manifest)   # target 해소·센티넬 거부를 그대로 재사용
    bud = turn_budget.budget(grade)
    base["intent"] = "delegate"
    base["task"] = task
    base["max_turns"] = bud["max_turns"]
    base["timeout_seconds"] = bud["timeout_seconds"]
    base["capabilities"] = list(capabilities or ["read", "execute", "edit", "write"])
    if resume_session_id:
        base["resume_session_id"] = resume_session_id
    return base


def record_attempt(doc: dict, *, context_id: str, grade: str, allocated: int,
                   result: dict, report=None) -> dict:
    att = {
        "attempt": len(doc.get("attempts") or []) + 1,
        "task_grade": {"assigned": grade,
                       "recommended": (report or {}).get("task_grade", {}).get("recommended")},
        "max_turns_allocated": allocated,
        # 모르면 null — 그럴듯한 값으로 채우면 Layer2 보정이 거짓 위에 선다.
        "max_turns_used": result.get("num_turns"),
        "budget_outcome": result.get("budget_outcome"),
        "session_id": result.get("session_id"),
        "control_status": result.get("status"),
        "reason_codes": result.get("reason_codes") or [],
        "status": (report or {}).get("status"),
        "phase": (report or {}).get("phase"),
        # SILENT_FALLBACK 금지: 메인이 대신 한 것은 여기에 적히지 않는다. 서브 산출만 집계한다.
        "sub_reported": bool(report),
    }
    doc["context_id"] = context_id
    doc.setdefault("attempts", []).append(att)
    return att


def surface_hitl(repo_root: str, context_id: str, report: dict) -> str | None:
    """서브가 스스로 표면화한 것만 릴레이한다(디스크 재스캔 ✗ — §2.4 push-attestation)."""
    hitl = (report or {}).get("hitl") or {}
    if not hitl.get("needed"):
        return None
    path = os.path.join(repo_root, TASKS_DIR_NAME, PENDING_HITL)
    doc = {"schema_version": 1, "pending": []}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            doc = json.load(f)
    doc["pending"] = [p for p in doc.get("pending", []) if p.get("context_id") != context_id]
    doc["pending"].append({
        "context_id": context_id,
        "request_id": hitl.get("request_id"),
        "source": hitl.get("source") or "sub-relay",
        "prompt": hitl.get("prompt"),
        "library_request": report.get("library_request") or [],
    })
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write("\n")
    return path


def parse_report(output: str):
    """서브 리포트는 JSON 1개다. 산문에 섞여 와도 마지막 JSON 객체를 집는다 — 못 찾으면 None."""
    if not isinstance(output, str):
        return None
    depth, start = 0, None
    best = None
    for i, ch in enumerate(output):
        if ch == "{":
            if depth == 0:
                start = i
            depth += 1
        elif ch == "}" and depth:
            depth -= 1
            if depth == 0 and start is not None:
                try:
                    best = json.loads(output[start:i + 1])
                except ValueError:
                    pass
    return best if isinstance(best, dict) else None


def _self_test() -> int:
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    with tempfile.TemporaryDirectory() as d:
        lp = ledger_path(d, "ctx-1")
        doc = load_ledger(lp)
        chk(doc["attempts"] == [], "빈 원장 초기화")

        # ① 정상 완료 → within_budget · 세션 이어붙이지 않음(완결된 세션은 잇지 않는다)
        record_attempt(doc, context_id="ctx-1", grade="L2", allocated=25,
                       result={"num_turns": 12, "budget_outcome": "within_budget",
                               "session_id": "s1", "status": "completed", "reason_codes": []},
                       report={"status": "completed", "phase": "config"})
        save_ledger(lp, doc)
        chk(json.load(open(lp))["attempts"][0]["max_turns_used"] == 12, "원장 4필드 기록")
        chk(latest_session_id(doc) is None, "완료된 attempt 의 세션은 재개 대상이 아니다")

        # ② input-required → 같은 세션을 잇는다
        record_attempt(doc, context_id="ctx-1", grade="L2", allocated=25,
                       result={"num_turns": 5, "budget_outcome": "within_budget",
                               "session_id": "s2", "status": "completed", "reason_codes": []},
                       report={"status": "input-required", "phase": "config"})
        chk(latest_session_id(doc) == "s2", "input-required → 같은 세션 재개(컨텍스트 재구축 회피)")

        # ③ 소진 → 세션을 잇지 않고, 다음 예산은 **더 크다**
        record_attempt(doc, context_id="ctx-1", grade="L2", allocated=25,
                       result={"num_turns": 25, "budget_outcome": "exhausted",
                               "session_id": "s3", "status": "execution_failed",
                               "reason_codes": ["NONZERO_EXIT"]},
                       report=None)
        chk(latest_session_id(doc) is None, "소진된 세션은 잇지 않는다(그 자리서 또 소진된다)")
        nb = next_budget(doc, "L2")
        chk(nb["max_turns"] > 25, f"소진 뒤 새 attempt 예산 증액 25 → {nb['max_turns']}")
        chk("escalated from 25" in nb["source"], "증액 사실이 출처에 남는다")

        # ④ SILENT_FALLBACK 금지: 리포트 없는 attempt 는 서브 산출로 집계되지 않는다
        chk(doc["attempts"][-1]["sub_reported"] is False,
            "서브 리포트 없는 attempt 는 sub_reported=false(메인 대행을 성공으로 집계 ✗)")

        # ⑤ HITL 표면화 — 서브가 스스로 needed 를 말한 것만
        p = surface_hitl(d, "ctx-1", {"hitl": {"needed": True, "request_id": "h1",
                                               "prompt": "어느 핀?"},
                                      "library_request": [{"question": "q", "kind": "version-pin"}]})
        chk(p and json.load(open(p))["pending"][0]["request_id"] == "h1", "HITL 표면화")
        p2 = surface_hitl(d, "ctx-2", {"hitl": {"needed": False}})
        chk(p2 is None, "needed=false 면 표면화하지 않는다(메인이 대신 만들지 않는다)")

        # ⑥ 리포트 파싱 — 산문에 섞여 와도 JSON 을 집는다 / 없으면 None
        chk(parse_report("어쩌고 {\"status\": \"completed\"} 끝")["status"] == "completed",
            "산문 속 JSON 리포트 추출")
        chk(parse_report("리포트 없음") is None, "JSON 이 없으면 None(추측 파싱 ✗)")
    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="메인↔서브 턴제 릴레이")
    ap.add_argument("--topology", choices=["single", "multi"])
    ap.add_argument("--manifest", default=None)
    ap.add_argument("--context-id")
    ap.add_argument("--grade", choices=list(turn_budget.GRADE_ORDER))
    ap.add_argument("--task", help="서브에 보낼 지시(또는 --task-file)")
    ap.add_argument("--task-file")
    ap.add_argument("--emit-only", action="store_true", help="request 만 조립해 출력(실행 ✗)")
    ap.add_argument("--repo-root", default=REPO)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    for req in ("topology", "context_id", "grade"):
        if not getattr(a, req):
            raise SystemExit(f"[relay] --{req.replace('_','-')} 필수")
    task = a.task
    if a.task_file:
        with open(a.task_file, encoding="utf-8") as f:
            task = f.read()
    if not task:
        raise SystemExit("[relay] --task 또는 --task-file 필수")

    manifest = a.manifest or os.path.join(REPO, "output", a.topology, "manifest.yaml")
    lp = ledger_path(a.repo_root, a.context_id)
    doc = load_ledger(lp)
    bud = next_budget(doc, a.grade)
    resume = latest_session_id(doc)
    req = build_request(a.topology, manifest, task, a.grade, resume_session_id=resume)
    req["max_turns"] = bud["max_turns"]
    req["timeout_seconds"] = bud["timeout_seconds"]

    if a.emit_only:
        json.dump(req, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0

    print(f"[relay] attempt={len(doc.get('attempts') or []) + 1} grade={a.grade} "
          f"max_turns={bud['max_turns']} ({bud['source']}) resume={resume or '(새 세션)'}")
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as tf:
        json.dump(req, tf, ensure_ascii=False)
        tmp = tf.name
    try:
        out = subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", tmp],
                             capture_output=True, text=True)
    finally:
        os.unlink(tmp)
    sys.stderr.write(out.stderr)
    try:
        result = json.loads(out.stdout)
    except ValueError:
        raise SystemExit(f"[relay] FAIL: agent_control 출력을 읽지 못했다 — {out.stdout[:300]}")

    report = parse_report(result.get("output") or "")
    att = record_attempt(doc, context_id=a.context_id, grade=a.grade,
                         allocated=bud["max_turns"], result=result, report=report)
    save_ledger(lp, doc)
    hp = surface_hitl(a.repo_root, a.context_id, report or {})

    print(f"[relay] 원장 → {lp}")
    print(f"[relay] control={att['control_status']} sub_status={att['status']} "
          f"turns={att['max_turns_used']}/{att['max_turns_allocated']} budget={att['budget_outcome']}")
    if hp:
        print(f"[relay] ⚠ 서브가 HITL 을 요청했다 → {hp} (사람 답변 후 같은 context_id 로 재개)")
    if att["budget_outcome"] == "exhausted":
        nxt = turn_budget.escalate(a.grade, bud["max_turns"])
        print(f"[relay] ⚠ 예산 소진(terminal). 자동 재시도하지 않는다 — 다음 attempt 는 "
              f"max_turns={nxt['max_turns']} 로 열고 직전 산출물을 prompt 로 실어라.")
        return 3
    if report is None:
        print("[relay] ⚠ 서브 리포트(JSON)를 찾지 못했다 — 산문만 왔다. 성공으로 집계하지 않는다.")
        return 4
    return 0 if att["status"] == "completed" else 2


if __name__ == "__main__":
    sys.exit(main())
