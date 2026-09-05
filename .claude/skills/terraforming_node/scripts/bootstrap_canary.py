#!/usr/bin/env python3
"""bootstrap_canary.py — §2.5 완료 게이트(model-less 카나리)의 **실행자**.

왜 이 파일이 생겼나(2026-09-03 · S1 · plan_26090317 P1):
    SKILL.md §2.5 는 `bootstrap_canary()` 를 호출 가능한 것처럼 적었지만, 저장소 전수에서 그 이름은
    **스키마 enum 값 2곳 + 산문 2곳**으로만 존재했다 — request JSON 을 manifest 에서 조립하는 코드가
    0개였다. `target.role=sub`·`transport=ssh`·`host`·`ssh_user`·`work_dir` 는 전부 manifest 에 있는데
    그것을 읽어 request 를 만드는 주체가 없었고, 유일한 경로는 "사람이 손으로 JSON 을 쓴다" 였으며
    그 절차는 어느 문서에도 없었다. 즉 §5 의 "카나리 미통과 시 done 선언 금지" 는 **실행자 없는 금지**
    였다 — 프로젝트 자신의 §2.7.1 규칙("게이트를 놓을 때는 그 처방을 누가 실행하는가를 먼저 적는다")을
    그 게이트가 어기고 있었다.

무엇을 하나:
    manifest → request JSON 조립(--emit) 또는 조립 후 agent_control 실행(--invoke)까지.
    조립은 결정론이고, 실행은 HITL 승인 뒤에만 부른다.

정체성 분기(헌법 불변식 A):
    카나리 Task 문구는 sub_mode 에 따라 다르다 — ray-worker 에게 "네 런타임 스킬 3종" 을 물으면
    거짓 자백을 유도하게 된다(그 모드는 스킬을 받지 않는다). sub_mode 는 node_role_contract 가 정한다.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
AGENT_CONTROL = os.path.join(REPO, ".claude", "policies", "runtime", "agent_control.py")
ROLE_CONTRACT = os.path.join(HERE, "node_role_contract.py")

# grade S(= inspect 카나리) 의 turn 예산. 매직상수가 아니라 grade 표의 값이며 출처를 함께 둔다
# (plan_26090317 §5 · 참고 프로젝트 `seed/hermes-subagent-control-plane/references/turn-budget-grades.md`).
GRADE = "S"
GRADE_MAX_TURNS = 10          # S = 8~12 의 중앙값. 실측으로 교정한다(원장 max_turns_used p90).
GRADE_TIMEOUT_S = 600
GRADE_SOURCE = "grade-table:S (plan_26090317 §5 · hermes turn-budget-grades)"

TASK_COMMON = (
    "부트스트랩 카나리다. 모델을 로드하지 말고, 빌드하지 말고, 아무 파일도 고치지 마라. "
    "다음을 스스로 확인해 **schema-valid JSON 리포트 1개**만 반환하라(raw 로그 나열 금지): "
    "① CLAUDE.md 를 읽고 네 정체성(sub_mode)과 작업공간 경로를 보고 "
    "② Agent_Card.json 의 capabilities.extensions[uri=urn:easy-vllm:ext:node-role:v1].params 4필드"
    "(sub_mode·sub_mode_source·rank·rank_source)와 signatures 존재 여부를 그대로 보고, "
    "output/<topology>/manifest.yaml 이 있으면 그 self_role·gpus_per_node 도 보고 "
    "③ .claude/rules/comms.md 의 리포트 계약을 읽었음을 보고 "
    "④ 로드된 런타임 스킬 목록(없으면 빈 배열)을 보고 "
    "⑤ 리포트를 .claude/schemas/task-report.schema.json 에 대조해 self_verification.schema_valid 에 담아라. "
    "phase=inspect · status=completed 로 끝내라. "
)
TASK_BY_MODE = {
    "a2a-agent": (
        "너는 A2A 원격 에이전트다(Ray 워커가 아니다). rank 는 없어야 하고, "
        "런타임 스킬 3종(vllm-recipe-explorer · adversarial-benchmark · upstream-version-watch)이 "
        "실제로 존재하는지 경로로 확인해 보고하라. "
        "추가로 grade_recommended(S|L0|L1|L2|L3|L4)와 needs_hitl 을 notes 에 담아라."
    ),
    "ray-worker": (
        "너는 멀티노드 Ray 워커다. 런타임 스킬은 받지 않는 것이 정상이므로 "
        "skills_loaded 가 비어 있어야 한다 — 비어 있음을 확인해 보고하라. rank 값을 함께 보고하라."
    ),
}


def _load_yaml_min(path: str) -> dict:
    """manifest 최소 파서. pyyaml 이 없어도 도는 것이 이 저장소의 계약이라 flat 파서를 쓴다."""
    try:
        import yaml  # noqa
        with open(path, encoding="utf-8") as f:
            return yaml.safe_load(f) or {}
    except ImportError:
        pass
    data: dict = {"nodes": []}
    node: dict | None = None
    with open(path, encoding="utf-8") as f:
        in_nodes = False
        for raw in f:
            line = raw.rstrip("\n")
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            if line.startswith("nodes:"):
                in_nodes = True
                continue
            if in_nodes:
                stripped = line.strip()
                if line.startswith(("  - ", "  -\t")) or stripped.startswith("- "):
                    node = {}
                    data["nodes"].append(node)
                    stripped = stripped[2:].strip()
                    if ":" in stripped:
                        k, _, v = stripped.partition(":")
                        node[k.strip()] = v.split("#", 1)[0].strip().strip('"')
                    continue
                if line.startswith("    ") and node is not None and ":" in stripped:
                    k, _, v = stripped.partition(":")
                    node[k.strip()] = v.split("#", 1)[0].strip().strip('"')
                    continue
                if not line.startswith(" "):
                    in_nodes = False
            if not line.startswith(" ") and ":" in line:
                k, _, v = line.partition(":")
                val = v.split("#", 1)[0].strip().strip('"')
                if val:
                    data[k.strip()] = val
    return data


def resolve_sub_mode(topology: str, manifest: str) -> tuple[str, str]:
    """sub_mode 판정은 node_role_contract 가 **정본**이다 — 여기서 role:sub 로 추론하지 않는다."""
    out = subprocess.run([sys.executable, ROLE_CONTRACT, "evaluate", "--topology", topology,
                          "--manifest", manifest, "--field", "sub_mode", "--format", "json"],
                         capture_output=True, text=True, timeout=30)
    if out.returncode != 0:
        raise SystemExit(f"[canary] FAIL: sub_mode 판정 실패(rc={out.returncode}) — "
                         f"{(out.stderr or out.stdout).strip()[:300]}")
    doc = json.loads(out.stdout)
    # evaluate --format json 은 {"<field>": {"value":..,"source":..}} 로 감싼다.
    inner = doc.get("sub_mode") if isinstance(doc.get("sub_mode"), dict) else doc
    return inner.get("value") or "", inner.get("source") or ""


def build_request(topology: str, manifest_path: str) -> dict:
    if not os.path.exists(manifest_path):
        raise SystemExit(f"[canary] FAIL: manifest 부재 — {manifest_path}")
    man = _load_yaml_min(manifest_path)
    sub = next((n for n in (man.get("nodes") or []) if str(n.get("role")) == "sub"), None)
    if sub is None:
        raise SystemExit("[canary] FAIL: manifest nodes[] 에 role: sub 가 없다 — 서브 미등록 상태에서는 "
                         "카나리를 조립할 수 없다(§2.5 는 전달 후 게이트다).")
    missing = [k for k in ("host", "ssh_user", "work_dir")
               if not sub.get(k) or str(sub[k]).startswith("__REQUIRED__")]
    if missing:
        raise SystemExit(f"[canary] FAIL: nodes[sub] 필수 필드 미해소 {missing} — "
                         f"추측해서 접속하지 않는다(틀린 계정/경로는 권한 평면을 바꾼다).")
    sub_mode, mode_source = resolve_sub_mode(topology, manifest_path)
    if sub_mode not in TASK_BY_MODE:
        raise SystemExit(f"[canary] FAIL: 알 수 없는 sub_mode={sub_mode!r}(출처 {mode_source}) — "
                         f"정체성이 정해지지 않으면 물어볼 내용도 정해지지 않는다.")
    return {
        "schema_version": 1,
        "provider": "claude_code",
        "intent": "bootstrap_canary",
        "model": "sonnet",
        "task": TASK_COMMON + TASK_BY_MODE[sub_mode],
        "target": {
            "role": "sub",
            "transport": "ssh",
            "host": str(sub["host"]),
            "ssh_user": str(sub["ssh_user"]),
            "work_dir": str(sub["work_dir"]),
        },
        "timeout_seconds": GRADE_TIMEOUT_S,
        "max_turns": GRADE_MAX_TURNS,
        "capabilities": ["read", "execute"],
    }


def _self_test() -> int:
    import tempfile
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    fixture = (
        "topology: multi\ncpu_arch: \"aarch64\"\n"
        "nodes:\n"
        "  - role: main\n    host: \"203.0.113.10\"\n    ssh_user: tester\n    work_dir: /srv/ws\n"
        "  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: tester\n    work_dir: /srv/ws\n"
    )
    with tempfile.TemporaryDirectory() as d:
        mp = os.path.join(d, "manifest.yaml")
        with open(mp, "w", encoding="utf-8") as f:
            f.write(fixture)
        man = _load_yaml_min(mp)
        sub = next((n for n in man["nodes"] if n.get("role") == "sub"), None)
        chk(sub and sub["host"] == "203.0.113.11" and sub["ssh_user"] == "tester"
            and sub["work_dir"] == "/srv/ws", "manifest nodes[sub] 5필드 해소")
        req = build_request("multi", mp)
        schema_path = os.path.join(REPO, ".claude", "schemas", "agent-control-request.schema.json")
        with open(schema_path, encoding="utf-8") as f:
            schema = json.load(f)
        req_keys = set(schema.get("required", []))
        chk(req_keys <= set(req), "request 가 스키마 required 를 모두 채움")
        chk(req["intent"] == "bootstrap_canary" and req["target"]["role"] == "sub"
            and req["target"]["transport"] == "ssh", "intent/target 계약")
        chk("ray" not in req["task"].lower() or "Ray 워커" in req["task"],
            "multi 카나리 문구가 ray-worker 정체성을 묻는다")
        chk(req["max_turns"] == GRADE_MAX_TURNS and GRADE_SOURCE.startswith("grade-table:"),
            "turn 예산이 매직상수가 아니라 grade 표에서 온다(출처 표기)")

        # 음성대조 ①: 서브 미등록이면 조립하지 않는다(추측 금지).
        mp2 = os.path.join(d, "single.yaml")
        with open(mp2, "w", encoding="utf-8") as f:
            f.write("topology: single\nnodes:\n  - role: main\n    host: \"h\"\n")
        try:
            build_request("single", mp2)
            chk(False, "서브 미등록 → fail-loud")
        except SystemExit:
            chk(True, "서브 미등록 → fail-loud")

        # 음성대조 ②: 센티넬이 남은 필드로는 접속을 조립하지 않는다.
        mp3 = os.path.join(d, "sentinel.yaml")
        with open(mp3, "w", encoding="utf-8") as f:
            f.write(fixture.replace("    ssh_user: tester\n    work_dir: /srv/ws\n"
                                    "  - role: sub", "    ssh_user: tester\n    work_dir: /srv/ws\n  - role: sub", 1)
                    .replace("  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: tester",
                             "  - role: sub\n    host: \"203.0.113.11\"\n    ssh_user: __REQUIRED__"))
        try:
            build_request("multi", mp3)
            chk(False, "__REQUIRED__ 센티넬 → fail-loud")
        except SystemExit:
            chk(True, "__REQUIRED__ 센티넬 → fail-loud")
    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="§2.5 model-less 카나리 request 조립기/실행자")
    ap.add_argument("--topology", choices=["single", "multi"])
    ap.add_argument("--manifest", default=None, help="기본 output/<topology>/manifest.yaml")
    ap.add_argument("--emit", metavar="PATH", help="request JSON 을 이 경로에 쓴다(기본: stdout)")
    ap.add_argument("--invoke", action="store_true",
                    help="조립 후 agent_control invoke 까지 실행한다(HITL 승인 뒤에만).")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if not a.topology:
        raise SystemExit("[canary] --topology 필수(토폴로지는 인터뷰가 정한다 — 브랜치로 추론하지 않는다)")
    manifest = a.manifest or os.path.join(REPO, "output", a.topology, "manifest.yaml")
    req = build_request(a.topology, manifest)
    blob = json.dumps(req, ensure_ascii=False, indent=2) + "\n"
    if a.emit:
        with open(a.emit, "w", encoding="utf-8") as f:
            f.write(blob)
        print(f"[canary] request → {a.emit} (grade={GRADE} max_turns={GRADE_MAX_TURNS} · {GRADE_SOURCE})")
    else:
        sys.stdout.write(blob)
    if not a.invoke:
        return 0
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as tf:
        tf.write(blob)
        tmp = tf.name
    try:
        return subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", tmp]).returncode
    finally:
        os.unlink(tmp)


if __name__ == "__main__":
    sys.exit(main())
