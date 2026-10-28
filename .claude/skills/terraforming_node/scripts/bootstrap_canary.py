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
    카나리 Task 문구는 sub_mode 에 따라 다르다 — ray-worker 에게 "네 런타임 스킬" 을 물으면
    거짓 자백을 유도하게 된다(그 모드는 스킬을 받지 않는다). sub_mode 는 node_role_contract 가 정한다.
    a2a-agent 에게 확인시킬 스킬 목록도 손으로 적지 않는다 — `TOOL_PLANE_BY_SUB_MODE` 에서 파생한다
    (2026-10-11 · plan_26101104: 런타임 3종 + hint-publisher 로 늘었을 때 문구가 따라가야 한다).
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import secrets
import shlex
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
AGENT_CONTROL = os.path.join(HERE, "agent_control.py")
ROLE_CONTRACT = os.path.join(HERE, "node_role_contract.py")
sys.path.insert(0, HERE)
import turn_budget  # noqa: E402

# 2026-09-05(G-A2): 여기 있던 `GRADE_MAX_TURNS=10`·`GRADE_TIMEOUT_S=600` 은 등급표의 S 행을
# 복제한 값이었다. 표가 사라졌으므로 카나리도 **예산을 선언받는다** — 부르는 쪽이 정한다.

TASK_COMMON = (
    "부트스트랩 카나리다. 모델을 로드하지 말고, 빌드하지 말고, 아무 파일도 고치지 마라. "
    "다음을 스스로 확인해 **schema-valid JSON 리포트 1개**만 반환하라(raw 로그 나열 금지): "
    "① CLAUDE.md 를 읽고 네 정체성(sub_mode)과 작업공간 경로를 보고 "
    "② output/<topology>/manifest.yaml 의 self_role·terraforming Flag·gpus_per_node 와 "
    "메인이 렌더 때 node_role_contract 로 해소해 CLAUDE.md·Agent_Card 에 실은 sub_mode·rank 를 그대로 보고"
    "(계약기 자체는 서브에 배달되지 않는다 — 없다고 보고하지 마라 · SSH는 endpoint 인증만 담당), "
    "Agent_Card.json 이 있으면 capability metadata로만 목록을 보고 "
    "③ .claude/rules/comms.md 의 리포트 계약을 읽었음을 보고 "
    "④ 로드된 런타임 스킬 목록(없으면 빈 배열)을 보고 "
    "⑤ 리포트를 .claude/schemas/task-report.schema.json 에 대조해 self_verification.schema_valid 에 담아라. "
    "phase=inspect · status=completed 로 끝내라. "
)
sys.path.insert(0, HERE)
import node_role_contract as _nrc  # noqa: E402  (형제 스크립트 — 스킬 목록의 정본)

TASK_BY_MODE = {
    "a2a-agent": (
        "너는 A2A 원격 에이전트다(Ray 워커가 아니다). rank 는 없어야 하고, "
        "런타임 스킬 %d종(%s)이 " % (len(_nrc.TOOL_PLANE_BY_SUB_MODE["a2a-agent"]),
                                    " · ".join(_nrc.TOOL_PLANE_BY_SUB_MODE["a2a-agent"])) +
        "실제로 존재하는지 경로로 확인해 보고하라. "
        "추가로 이 정도 과업에 필요하다고 보는 턴 수(budget_recommendation)와 needs_hitl 을 notes 에 담아라."
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


def _backend_default_model(backend: str | None, provider: str = "claude_code") -> str:
    """백엔드의 기본 모델 선언을 **어댑터에서 읽는다**(사본 ✗ · 단일 권위)."""
    import importlib.util, os as _os
    import agent_control
    if provider not in agent_control.KNOWN_PROVIDERS:
        raise SystemExit(f"[canary] FAIL: unknown provider {provider!r}")
    _p = _os.path.join(HERE, "providers", provider + ".py")
    _spec = importlib.util.spec_from_file_location("_canary_provider", _p)
    _m = importlib.util.module_from_spec(_spec)
    _spec.loader.exec_module(_m)
    defaults = _m.BACKEND_DEFAULT_MODEL
    key = backend or next(iter(defaults))
    if key not in defaults:
        raise SystemExit(f"[canary] FAIL: backend {key!r} unsupported by {provider}")
    return defaults[key]


def backend_choices() -> list:
    """CLI `--backend` 선택지. **전송 스키마 enum 을 읽는다**(2026-09-15) — 종전 relay 의
    `choices=[...]` 손사본은 백엔드가 늘 때마다 갈라지는 자리였다(매직넘버 결함 칸)."""
    with open(turn_budget.REQUEST_SCHEMA, encoding="utf-8") as f:
        enum = (((json.load(f).get("properties") or {}).get("backend") or {}).get("enum"))
    if not isinstance(enum, list) or not enum:
        raise SystemExit(f"[canary] FAIL: 요청 스키마에서 backend enum 을 읽지 못했다 — "
                         f"{turn_budget.REQUEST_SCHEMA}")
    return enum


DEFAULT_CANARY_MODEL = "sonnet"    # 카나리 1왕복의 **기본 선언값**이지 게이트가 아니다(--model 로 바꾼다)
PROBE_REL = ".claude/skills/terraforming_node/scripts/harness_permission_probe.py"
PLAN_PREFIX = "HARNESS_CANARY_PLAN="


def permission_plan(request: dict) -> dict | None:
    """Bounded plan carried by the existing request, not a parallel readiness flag."""
    lines = [x[len(PLAN_PREFIX):] for x in request.get("task", "").splitlines()
             if x.startswith(PLAN_PREFIX)]
    if len(lines) != 1:
        return None
    try:
        p = json.loads(lines[0])
    except ValueError:
        return None
    if not isinstance(p, dict) or p.get("phase") != "permissions":
        return None
    if (p.get("schema_version") != 1 or p.get("topology") not in ("single", "multi")
            or not isinstance(p.get("nonce"), str) or len(p["nonce"]) != 32
            or any(c not in "0123456789abcdef" for c in p["nonce"])
            or p.get("target") != request.get("target")):
        return None
    target = request["target"]
    expected = ["python3", "-B", PROBE_REL, "--workspace", target["work_dir"],
                "--topology", p["topology"], "--role", target["role"], "--nonce", p["nonce"]]
    return p if p.get("argv") == expected else None


def _permission_task(topology: str, target: dict, nonce: str, provider: str) -> str:
    argv = ["python3", "-B", PROBE_REL, "--workspace", target["work_dir"],
            "--topology", topology, "--role", target["role"], "--nonce", nonce]
    plan = {"schema_version": 1, "phase": "permissions", "nonce": nonce,
            "topology": topology, "target": target, "argv": argv}
    build_outcome = "denied" if topology == "multi" and target["role"] == "sub" else "allowed"
    import harness_permission_probe as probe
    path_expectations = probe.protected_file_expectations(target["role"])
    enforcement = (
        "os_enforcement의 protected:* 기대값은 역할에 따라 "
        + json.dumps(path_expectations, ensure_ascii=False, separators=(",", ":"))
        + "이고 각각 preserved=true여야 한다. 메인의 허용은 하네스 저작 권한 확인이며 "
        "서브의 예상된 차단은 정상 음성대조다. 예상된 차단을 errors나 HITL로 보고하지 마라. "
        f"이 역할의 buildkit:*는 {build_outcome}이고 preserved=true여야 한다. "
        if provider == "codex_cli" else
        "os_enforcement는 OS write-open 관측일 뿐 Claude 도구/managed 정책 집행의 증명이 아니다. "
        "이 표면만으로 업무 실패나 HITL을 만들지 말고 실제 관측과 미입증 한계를 notes에 남겨라. "
    )
    return (
        "업무 권한 카나리다. serving 모델·GPU·빌드·sudo·인증변경·원격 git·정책변경을 하지 마라. "
        "이 요청의 native 도구에서 아래 명령을 그대로 정확히 1회 실행하라. "
        "helper는 전용 scratch/독점 marker만 쓰고 회수하며 기존 canonical 파일에는 쓰지 않는다. "
        "도서관 query는 기존 파생 cache를 만들 수 있다. Docker는 version 조회만 한다. "
        "helper를 읽기만 하거나 결과를 만들어 쓰지 마라. 도구가 거부하면 우회·재실행하지 말고 보고하라.\n"
        + shlex.join(argv) + "\n"
        "마지막 응답은 task-report.schema.json에 맞는 JSON 객체 1개다. "
        f"task_id=context_id='harness-canary-{nonce}', turn=1, node_id='{target['role']}', phase='inspect'. "
        "실제 수행만 self_verification.checks_run에 적어라. "
        "functional 검사는 모두 allowed여야 하며, model_source=ephemeral인 model_asset_read만 "
        "not_applicable이 정상이다. "
        + enforcement +
        "필요한 기능의 거부·미관측 또는 기대 결과 불일치는 input-required와 hitl로 보고한다. "
        "모든 해당 조건을 만족하면 status=completed, errors=[], hitl.needed=false, "
        "hitl.blocking=false로 보고하라. 이 제한된 카나리 밖의 build/run·원격 hint 발행·재개가 "
        "미검증인 것은 notes의 한계이며 그 자체로 추가 승인 요청을 만들지 않는다. "
        "helper stdout은 native 도구 결과에 남긴다. notes의 자기주장만으로 검증을 대신하지 않는다.\n"
        + PLAN_PREFIX + json.dumps(plan, ensure_ascii=False, separators=(",", ":"))
    )


def _command_matches(command, argv) -> bool:
    if not isinstance(command, str):
        return False
    try:
        tokens = shlex.split(command)
        if tokens and tokens[0] in ("/bin/bash", "bash", "/bin/sh", "sh"):
            if len(tokens) == 3 and tokens[1] in ("-lc", "-c"):
                tokens = shlex.split(tokens[2])
        return tokens == argv
    except ValueError:
        return False


def _tool_receipts(result: dict, argv) -> list[str]:
    """Only native completed tool results; never assistant text or final notes."""
    receipts, uses, seen_ids = [], {}, set()
    for e in result.get("raw_events") or []:
        if not isinstance(e, dict):
            continue
        item = e.get("item")
        if (result.get("provider") == "codex_cli" and e.get("type") == "item.completed" and isinstance(item, dict)
                and item.get("type") == "command_execution"
                and item.get("status") == "completed" and item.get("exit_code") == 0
                and _command_matches(item.get("command"), argv)
                and isinstance(item.get("aggregated_output"), str)):
            receipts.append(item["aggregated_output"])
        if result.get("provider") != "claude_code":
            continue
        message = e.get("message")
        if not isinstance(message, dict):
            continue
        for block in message.get("content") or []:
            if not isinstance(block, dict):
                continue
            if e.get("type") == "assistant" and block.get("type") == "tool_use":
                ident = block.get("id")
                if not isinstance(ident, str) or not ident or ident in seen_ids:
                    return []
                seen_ids.add(ident)
            if (e.get("type") == "assistant" and block.get("type") == "tool_use"
                    and block.get("name") == "Bash" and isinstance(block.get("input"), dict)
                    and _command_matches(block["input"].get("command"), argv)):
                uses[block.get("id")] = True
            if (e.get("type") == "user" and block.get("type") == "tool_result"
                    and block.get("tool_use_id") in uses and block.get("is_error") is not True):
                body = block.get("content")
                if isinstance(body, list):
                    body = "\n".join(b.get("text", "") for b in body
                                     if isinstance(b, dict) and b.get("type") == "text")
                if isinstance(body, str):
                    receipts.append(body)
    return receipts


def verify_result(request: dict, result: dict) -> dict:
    """Three existing evidence layers, with no disk/SSH rescan and no fallback executor."""
    import agent_control as ac
    if not isinstance(request, dict) or not isinstance(result, dict):
        return {"status": "input-required", "phase": "inspect", "reason": "REQUEST_OR_RESULT_NOT_OBJECT"}
    problems = ac.request_schema_violations(request)
    problems += ac._schema_violations(result, ac._load_schema(ac.RESULT_SCHEMA_PATH))
    if not ac.request_schema_violations(request):
        problems += ac._result_contract_violations(result, request)
    native_ok = not problems and (result.get("status"), result.get("exit_code")) == ("completed", 0)
    native_ok = native_ok and result.get("provider") == request.get("provider")
    native_ok = native_ok and result.get("model_requested") == request.get("model")
    try:
        report = json.loads(result.get("output") or "")
    except (ValueError, TypeError):
        report = None
    schema = ac._load_schema(Path(HERE).parent / "sub_node/task-report.schema.json")
    schema_errors = ac._schema_violations(report, schema)
    report_ok = not schema_errors and report.get("phase") == "inspect" and report.get("status") == "completed"
    plan = permission_plan(request)
    domain = {"status": "unproven", "reasons": []}
    enforcement = {"status": "unproven", "surface": "os_write_open", "limitations": []}
    if plan is None:
        domain["reasons"].append("DISCOVERY_ONLY_NO_WORK_PERMISSION_OBSERVATIONS")
    else:
        expected_context = "harness-canary-" + str(plan.get("nonce"))
        report_ok = report_ok and all(report.get(k) == expected_context for k in ("task_id", "context_id"))
        report_ok = report_ok and report.get("node_id") == request["target"]["role"]
        import harness_permission_probe as probe
        packets = []
        for receipt in _tool_receipts(result, plan.get("argv")):
            for line in receipt.splitlines():
                if line.startswith(probe.MARKER):
                    try:
                        packets.append(json.loads(line[len(probe.MARKER):]))
                    except ValueError:
                        packets.append(None)
        if len(packets) != 1 or not isinstance(packets[0], dict):
            domain["reasons"].append("EXACT_NATIVE_PROBE_RECEIPT_ABSENT_OR_AMBIGUOUS")
        else:
            packet = packets[0]
            bound = (packet.get("schema_version") == probe.SCHEMA_VERSION
                     and packet.get("nonce") == plan.get("nonce")
                     and packet.get("topology") == plan.get("topology")
                     and packet.get("role") == request["target"]["role"]
                     and packet.get("workspace") == request["target"]["work_dir"]
                     and packet.get("provenance") == "observed_local_process")
            checks = packet.get("checks")
            valid = isinstance(checks, list) and all(isinstance(c, dict) and isinstance(c.get("name"), str) for c in checks)
            by_name = {c["name"]: c for c in checks} if valid else {}
            valid = valid and len(by_name) == len(checks)
            required = {"scratch_write_cleanup", "scratch_local_git", "scratch_removed",
                        "workspace_git_metadata", "docker_daemon_read", "docker_compose_read", "library_query",
                        "model_asset_read", "buildkit_declaration"}
            if plan.get("topology") == "single" and request["target"]["role"] == "sub":
                required.update(("hint_clone_write", "hint_clone_git_discovery", "hint_clone_write_git_1"))
            functional = bound and valid and required <= set(by_name)
            functional = functional and all(c.get("outcome") == "allowed" or (
                c.get("name") == "model_asset_read" and c.get("outcome") == "not_applicable"
                and c.get("model_source") == "ephemeral") for c in checks if c.get("axis") == "functional")
            functional = functional and all(by_name[n].get("axis") == "functional" for n in required)
            domain.update(status="pass" if functional else "unproven", observations=packet)
            if not functional:
                domain["reasons"].append("REQUIRED_ROLE_WORK_UNOBSERVED_OR_FAILED")
            build_files = packet.get("build_files")
            build_valid = (isinstance(build_files, list) and all(isinstance(n, str) for n in build_files)
                           and len(build_files) == len(set(build_files))
                           and set(probe.BUILD_FILES) <= set(build_files)
                           and bool(set(probe.DOCKERFILES) & set(build_files))
                           and set(build_files) <= set(probe.BUILD_FILES + probe.DOCKERFILES))
            os_required = {"buildkit:" + n: ("denied" if request["target"]["role"] == "sub"
                            and plan.get("topology") == "multi" else "allowed") for n in (build_files if build_valid else [])}
            os_required.update({"protected:" + n: outcome for n, outcome in
                                probe.protected_file_expectations(request["target"]["role"]).items()})
            os_ok = bound and valid and build_valid and all(by_name.get(n, {}).get("outcome") == outcome
                    and by_name[n].get("preserved") is True and by_name[n].get("axis") == "os_enforcement"
                    for n, outcome in os_required.items())
            if request.get("provider") == "codex_cli":
                enforcement["status"] = "pass" if os_ok else "unproven"
                if request["target"]["role"] == "main":
                    enforcement["limitations"].append("Main canonical write-open checks prove role access, not denial outside the approved workspace or effective profile selection")
            else:
                enforcement["limitations"].append("Claude tool/managed deny must be observed at its native tool surface; Python write-open does not establish that boundary")
    complete = native_ok and report_ok and domain["status"] == "pass"
    if request.get("provider") == "codex_cli":
        complete = complete and enforcement["status"] == "pass"
    return {"status": "completed" if complete else "input-required", "phase": "inspect",
            "native_cli": {"status": "pass" if native_ok else "unproven", "schema_errors": problems,
                           "model_requested": request.get("model"), "model_observed": result.get("model_used", [])},
            "task_report": {"status": "pass" if report_ok else "unproven", "schema_errors": schema_errors},
            "domain_evidence": domain, "enforcement_evidence": enforcement,
            "limitations": ["No model serving, Docker build/run, remote push, or native resume proof",
                            "completed is this canary's bounded evidence, not performance/promotion completion"]}


def build_request(topology: str, manifest_path: str, *,
                  max_turns: int = None, timeout_seconds: int = None,
                  budget_source: str = None, model: str = None,
                  backend: str = None, provider: str = "claude_code",
                  max_dispatches: int | None = None, sandbox_mode: str | None = None,
                  permission_profile: str | None = None, phase: str = "discovery",
                  target_role: str = "sub", main_work_dir: str | None = None,
                  reasoning_effort: str | None = None) -> dict:
    """카나리 request 조립. 예산은 **선언받는다**(기본값 ✗ — `turn_budget` 참조).

    `model` 은 선언이다(2026-09-05 · G-A1): 어댑터가 모델을 막지 않으므로 여기 값도 게이트가 아니라
    기본 선언이며, 실제로 무엇이 돌았는지는 결과의 `model_used` 가 말한다.
    """
    if not os.path.exists(manifest_path):
        raise SystemExit(f"[canary] FAIL: manifest 부재 — {manifest_path}")
    man = _load_yaml_min(manifest_path)
    if phase not in ("discovery", "permissions") or target_role not in ("main", "sub"):
        raise SystemExit("[canary] FAIL: unknown phase/target")
    sub = next((n for n in (man.get("nodes") or []) if str(n.get("role")) == "sub"), None)
    if target_role == "sub" and sub is None:
        raise SystemExit("[canary] FAIL: manifest nodes[] 에 role: sub 가 없다 — 서브 미등록 상태에서는 "
                         "카나리를 조립할 수 없다(§2.5 는 전달 후 게이트다).")
    missing = [k for k in ("host", "ssh_user", "work_dir")
               if not sub.get(k) or str(sub[k]).startswith("__REQUIRED__")] if target_role == "sub" else []
    if target_role == "sub" and missing:
        raise SystemExit(f"[canary] FAIL: nodes[sub] 필수 필드 미해소 {missing} — "
                         f"추측해서 접속하지 않는다(틀린 계정/경로는 권한 평면을 바꾼다).")
    unit = "wall_time" if provider == "codex_cli" else "native_turns"
    bud = turn_budget.declare(max_turns, timeout_seconds, source=budget_source or "",
                              unit=unit, max_dispatches=max_dispatches)
    sub_mode, mode_source = resolve_sub_mode(topology, manifest_path) if target_role == "sub" else ("main", "explicit main target")
    if target_role == "sub" and sub_mode not in TASK_BY_MODE:
        raise SystemExit(f"[canary] FAIL: 알 수 없는 sub_mode={sub_mode!r}(출처 {mode_source}) — "
                         f"정체성이 정해지지 않으면 물어볼 내용도 정해지지 않는다.")
    request = {
        "schema_version": 2 if provider == "codex_cli" else 1,
        "provider": provider,
        "intent": "bootstrap_canary",
        # 2026-09-08: 백엔드별 기본 모델은 **어댑터가 소유한다**(BACKEND_DEFAULT_MODEL).
        #   종전에는 `k3[1m]` 리터럴이 여기 손으로 적혀 있어, 백엔드가 늘면 갈라지는 자리였다.
        "model": model or _backend_default_model(backend, provider),
        "task": (TASK_COMMON + TASK_BY_MODE[sub_mode] if target_role == "sub" else
                 "메인 discovery 카나리다. serving 모델·GPU·빌드·파일변경을 하지 마라. "
                 "CLAUDE.md와 AGENTS.md의 정본·메인 역할·output/<topology>/manifest.yaml을 확인하고 "
                 ".claude/skills/terraforming_node/sub_node/task-report.schema.json 정본에 맞는 "
                 "phase=inspect schema-valid JSON 리포트 1개를 반환하라. 서브 comms나 sub_mode를 요구하지 않는다."),
        "target": ({
            "role": "sub",
            "transport": "ssh",
            "host": str(sub["host"]),
            "ssh_user": str(sub["ssh_user"]),
            "work_dir": str(sub["work_dir"]),
        } if target_role == "sub" else {"role": "main", "transport": "local", "work_dir": os.path.abspath(main_work_dir or REPO)}),
        "capabilities": ["read", "execute"],
        **({"backend": backend} if backend else {}),  # 미지정=anthropic 기본(스키마 선택 필드)
        # 예산은 선언된 값 두 개만 싣는다 — 요청 스키마는 `additionalProperties: false` 라
        # 출처(source)를 여기 실으면 전송 자체가 거부된다(2026-09-05 자체검사가 잡음).
        "timeout_seconds": bud["timeout_seconds"],
        "max_turns": bud["max_turns"],
    }
    if reasoning_effort is not None:
        request["reasoning_effort"] = reasoning_effort
    if provider == "codex_cli":
        request.update(budget_unit=unit, max_dispatches=bud["max_dispatches"])
        if sandbox_mode and permission_profile:
            raise SystemExit("[canary] FAIL: choose sandbox_mode OR permission_profile")
        if permission_profile:
            request["permission_profile"] = permission_profile
        else:
            request["sandbox_mode"] = sandbox_mode or "read-only"
        request["task"] = "AGENTS.md 를 먼저 읽고 그 문서가 지정한 정본 지침을 읽어라. " + request["task"]
    if phase == "permissions":
        if not model:
            raise SystemExit("[canary] FAIL: permissions phase requires explicit --model")
        if provider == "codex_cli" and (sandbox_mode or permission_profile != "vllm-" + target_role):
            raise SystemExit("[canary] FAIL: permissions phase requires the exact named role profile, no legacy sandbox")
        request["task"] = _permission_task(topology, request["target"], secrets.token_hex(16), provider)
        request["capabilities"] = ["read", "execute", "edit", "write"]
        if provider == "claude_code":
            request["capture_native_events"] = True
    return request


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
        req = build_request("multi", mp, max_turns=10, timeout_seconds=600,
                            budget_source="선언: 카나리 1왕복(인스펙트 전용 · 파일 수정 없음)")
        # ★ 픽스처를 실물 폭으로: required 포함 여부만 보던 종전 검사는 **미지 필드**를 못 봤다.
        #   실제 전송 게이트(agent_control 의 구조 검증기)를 그대로 불러 대조한다.
        import importlib.util as _ilu
        _spec = _ilu.spec_from_file_location("_ac_validator", AGENT_CONTROL)
        _ac = _ilu.module_from_spec(_spec)
        _spec.loader.exec_module(_ac)
        viol = _ac.request_schema_violations(req)
        chk(viol == [], f"request 가 전송 스키마를 그대로 통과한다(위반 {viol})")
        chk(req["intent"] == "bootstrap_canary" and req["target"]["role"] == "sub"
            and req["target"]["transport"] == "ssh", "intent/target 계약")
        chk("ray" not in req["task"].lower() or "Ray 워커" in req["task"],
            "multi 카나리 문구가 ray-worker 정체성을 묻는다")
        chk(req["max_turns"] == 10 and req["timeout_seconds"] == 600 and "source" not in req,
            "turn 예산이 **선언**으로 들어오고 출처는 요청 밖에 남는다(등급표 ✗)")
        # 음성대조 ⓪: 예산을 선언하지 않으면 조립되지 않는다(기본값이 없다).
        try:
            build_request("multi", mp)
            chk(False, "예산 미선언 → fail-loud")
        except SystemExit as _e:
            chk("선언되지 않았다" in str(_e), "예산 미선언 → fail-loud(기본값을 만들지 않는다)")

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
    ap.add_argument("--phase", choices=("discovery", "permissions"), default="discovery",
                    help="기존 읽기 discovery 또는 실제 native 도구 업무권한 관측")
    ap.add_argument("--target", choices=("main", "sub"), default="sub")
    ap.add_argument("--main-work-dir", help="main/local 대상 작업공간(기본 이 checkout); sub는 manifest만 사용")
    ap.add_argument("--verify-request", help="보존 request JSON과 --verify-result를 오프라인 검증(실행 없음)")
    ap.add_argument("--verify-result", help="보존 agent_control result JSON; native/schema/domain 분리 판정")
    ap.add_argument("--emit", metavar="PATH", help="request JSON 을 이 경로에 쓴다(기본: stdout)")
    ap.add_argument("--max-turns", type=int, default=None,
                    help="이 카나리에 배정할 턴 예산(선언 필수 — 기본값 없음)")
    ap.add_argument("--provider", choices=("claude_code", "codex_cli"), default="claude_code")
    ap.add_argument("--max-dispatches", type=int, default=None,
                    help="Codex wall-time 계약의 명시적 외부 위임 상한")
    permissions = ap.add_mutually_exclusive_group()
    permissions.add_argument("--sandbox-mode", choices=("read-only", "workspace-write"))
    permissions.add_argument("--permission-profile")
    ap.add_argument("--timeout-seconds", type=int, default=None,
                    help="매달림을 잡는 상한(scope ⊥ budget — 예산과 별개 노브)")
    ap.add_argument("--budget-source", default=None, help="그 예산을 그렇게 정한 근거(필수)")
    ap.add_argument("--reasoning-effort", choices=("none", "minimal", "low", "medium", "high", "xhigh", "max"))
    ap.add_argument("--model", default=None,
                    help=f"위임 모델 선언(기본 = 백엔드의 기본 모델 · 어댑터는 모델을 막지 않는다)")
    ap.add_argument("--backend", choices=backend_choices(), default=None,
                    help="서브 러너 백엔드(생략 = anthropic). 러너를 바꿔 A2A 평면을 검증할 때 쓴다")
    ap.add_argument("--invoke", action="store_true",
                    help="조립 후 agent_control invoke 까지 실행한다(HITL 승인 뒤에만).")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if a.verify_request or a.verify_result:
        if not (a.verify_request and a.verify_result) or a.invoke or a.emit:
            ap.error("--verify-request/--verify-result require both, without --invoke/--emit")
        with open(a.verify_request, encoding="utf-8") as f:
            request = json.load(f)
        with open(a.verify_result, encoding="utf-8") as f:
            result = json.load(f)
        verification = verify_result(request, result)
        print(json.dumps(verification, ensure_ascii=False, indent=2))
        return 0 if verification["status"] == "completed" else 4
    if not a.topology:
        raise SystemExit("[canary] --topology 필수(토폴로지는 인터뷰가 정한다 — 브랜치로 추론하지 않는다)")
    manifest = a.manifest or os.path.join(REPO, "output", a.topology, "manifest.yaml")
    req = build_request(a.topology, manifest, max_turns=a.max_turns,
                        timeout_seconds=a.timeout_seconds, budget_source=a.budget_source,
                        model=a.model, backend=a.backend, provider=a.provider,
                        max_dispatches=a.max_dispatches, sandbox_mode=a.sandbox_mode,
                        permission_profile=a.permission_profile, phase=a.phase,
                        target_role=a.target, main_work_dir=a.main_work_dir,
                        reasoning_effort=a.reasoning_effort)
    blob = json.dumps(req, ensure_ascii=False, indent=2) + "\n"
    if a.emit:
        with open(a.emit, "w", encoding="utf-8") as f:
            f.write(blob)
        print(f"[canary] request → {a.emit} (max_turns={req['max_turns']} "
              f"timeout={req['timeout_seconds']}s · {a.budget_source})")
    elif not (a.invoke and a.phase == "permissions"):
        sys.stdout.write(blob)
    if not a.invoke:
        return 0
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as tf:
        tf.write(blob)
        tmp = tf.name
    try:
        if a.phase == "permissions":
            # Adapter remains the only native executor. Do not retry, switch provider,
            # substitute a legacy policy, or read the remote filesystem after failure.
            invoked = subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", tmp],
                                     capture_output=True, text=True)
            try:
                result = json.loads(invoked.stdout)
            except ValueError:
                print(json.dumps({"status": "input-required", "phase": "inspect",
                                  "native_cli": {"status": "unproven", "returncode": invoked.returncode},
                                  "reason": "NATIVE_RESULT_NOT_JSON", "raw_stdout": invoked.stdout,
                                  "raw_stderr": invoked.stderr}, ensure_ascii=False, indent=2))
                return 4
            verification = verify_result(req, result)
            print(json.dumps({"request": req, "native_result": result,
                              "verification": verification}, ensure_ascii=False, indent=2))
            return 0 if verification["status"] == "completed" else 4
        return subprocess.run([sys.executable, AGENT_CONTROL, "invoke", "--request", tmp]).returncode
    finally:
        os.unlink(tmp)


if __name__ == "__main__":
    sys.exit(main())
