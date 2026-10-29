#!/usr/bin/env python3
"""terraforming_node-owned provider-neutral agent-control orchestrator (Phase 6, plan_26072506).

Ownership (plan_26093022 Q3): moved from `.claude/policies/runtime/` -- the base layer never runs this in an
operational path; its callers are this skill's relay/canary scripts. Each provider owns its
CLI syntax: the orchestrator validates and routes without emitting invocation grammar.

Reads a request JSON conforming to `.claude/schemas/agent-control-request.schema.json`, dispatches
it to the sibling `providers/` adapter namespace (v1 Claude Code; v2 Codex CLI), and prints a stable
(`json.dumps(..., sort_keys=True, indent=2)`) result JSON conforming to
`.claude/schemas/agent-control-result.schema.json` to stdout, exiting with `result["exit_code"]`.

This file stays provider-neutral: it never embeds Claude-specific argv/metadata parsing (that
lives entirely in the sibling provider adapter).  The enforcement is this module's own `--self-test`
(`_test_agent_provider_boundary`), which `.claude/policies/runtime/runtime_selftest.py` runs as one subprocess.

Exit-code <-> status <-> reason_codes table (single source of truth):
    0   completed              reason_codes == []
    2   invalid_request        REQUEST_SCHEMA_INVALID
    4   execution_failed       NONZERO_EXIT | IS_ERROR | PERMISSION_DENIED
    5   malformed_output       MALFORMED_JSON
    124 timeout                TIMEOUT

Usage:
    python3 .claude/skills/terraforming_node/scripts/agent_control.py invoke --request <path-to-request.json>
    python3 .claude/skills/terraforming_node/scripts/agent_control.py --self-test
    python3 .claude/skills/terraforming_node/scripts/agent_control.py --help
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shlex
import sys
import tempfile
from pathlib import Path

SCHEMA_VERSION_DEFAULT = 1
RUNTIME_DIR = Path(__file__).resolve().parent
REPO_ROOT = Path(__file__).resolve().parents[4]
REQUEST_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-request.schema.json"
RESULT_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-result.schema.json"
PROVIDERS_DIR = RUNTIME_DIR / "providers"

KNOWN_INTENTS = ("delegate", "probe", "bootstrap_canary")
KNOWN_PROVIDERS = ("claude_code", "codex_cli")

EXIT_INVALID_REQUEST = 2


# =============================================================================
# small self-contained Draft-07-subset structural validator (type/enum/minLength/minimum/required/
# properties/additionalProperties(bool)/items) -- the two Phase 6 schemas are flat and do not need
# $ref/allOf/if-then-else. Modeled on the sibling completion_gate.py validator.
# =============================================================================
def _json_type_matches(instance, type_name: str) -> bool:
    if type_name == "object":
        return isinstance(instance, dict)
    if type_name == "array":
        return isinstance(instance, list)
    if type_name == "string":
        return isinstance(instance, str)
    if type_name == "integer":
        return isinstance(instance, int) and not isinstance(instance, bool)
    if type_name == "number":
        return isinstance(instance, (int, float)) and not isinstance(instance, bool)
    if type_name == "boolean":
        return isinstance(instance, bool)
    if type_name == "null":
        return instance is None
    return False


def _schema_violations(instance, schema: dict, path: str = "$") -> list[str]:
    out: list[str] = []
    if "oneOf" in schema:
        matches = sum(not _schema_violations(instance, branch, path) for branch in schema["oneOf"])
        if matches != 1:
            out.append(f"{path}: expected exactly one oneOf branch, matched {matches}")
    if "type" in schema:
        allowed = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(_json_type_matches(instance, t) for t in allowed):
            out.append(f"{path}: expected type {allowed}, got {type(instance).__name__} ({instance!r})")
            return out
    if "enum" in schema and instance not in schema["enum"]:
        out.append(f"{path}: {instance!r} not in enum {schema['enum']!r}")
    if "const" in schema and instance != schema["const"]:
        out.append(f"{path}: expected const {schema['const']!r}, got {instance!r}")
    if isinstance(instance, str) and "minLength" in schema and len(instance) < schema["minLength"]:
        out.append(f"{path}: length {len(instance)} below minLength {schema['minLength']}")
    if isinstance(instance, (int, float)) and not isinstance(instance, bool) and "minimum" in schema \
            and instance < schema["minimum"]:
        out.append(f"{path}: {instance!r} below minimum {schema['minimum']}")
    if isinstance(instance, (int, float)) and not isinstance(instance, bool) and "maximum" in schema \
            and instance > schema["maximum"]:
        out.append(f"{path}: {instance!r} above maximum {schema['maximum']}")
    if isinstance(instance, list) and "items" in schema:
        for i, item in enumerate(instance):
            out.extend(_schema_violations(item, schema["items"], f"{path}[{i}]"))
    if isinstance(instance, list) and "minItems" in schema and len(instance) < schema["minItems"]:
        out.append(f"{path}: {len(instance)} items below minItems {schema['minItems']}")
    if isinstance(instance, list) and "maxItems" in schema and len(instance) > schema["maxItems"]:
        out.append(f"{path}: {len(instance)} items above maxItems {schema['maxItems']}")
    if isinstance(instance, list) and schema.get("uniqueItems") is True:
        if len({json.dumps(item, sort_keys=True) for item in instance}) != len(instance):
            out.append(f"{path}: duplicate items forbidden")
    if isinstance(instance, dict):
        props = schema.get("properties", {})
        for key in schema.get("required", []):
            if key not in instance:
                out.append(f"{path}.{key}: missing required property")
        if schema.get("additionalProperties") is False:
            for key in instance:
                if key not in props:
                    out.append(f"{path}.{key}: unknown property (additionalProperties: false)")
        for key, subschema in props.items():
            if key in instance:
                out.extend(_schema_violations(instance[key], subschema, f"{path}.{key}"))
    return out


def _load_schema(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _load_provider(provider_name: str):
    """Dynamically loads sibling providers/<provider_name>.py -- pure routing, no provider-specific
    argv/metadata logic lives here."""
    provider_path = PROVIDERS_DIR / f"{provider_name}.py"
    spec = importlib.util.spec_from_file_location(f"agent_control_provider_{provider_name}", provider_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _request_contract_violations(request: dict) -> list[str]:
    """Cross-field target contract kept provider-neutral: main is local, sub is SSH, and every
    invocation has an explicit workspace; SSH additionally requires a host."""
    target = request["target"]
    role = target["role"]
    transport = target["transport"]
    out = []
    if request.get("capture_native_events") is True and request["intent"] not in ("probe", "bootstrap_canary"):
        out.append("$.capture_native_events: native evidence capture is diagnostic-only")
    if (role, transport) not in (("main", "local"), ("sub", "ssh")):
        out.append("$.target: role/transport mismatch")
    if not isinstance(target.get("work_dir"), str) or not target["work_dir"]:
        out.append("$.target.work_dir: required")
    if transport == "ssh" and (not isinstance(target.get("host"), str) or not target["host"]):
        out.append("$.target.host: required for ssh")
    # 2026-09-03(S5 · plan_26090317 P1): ssh_user 가 선택이라 provider 의 _ssh_destination 이 host 단독
    #   으로 접속했다 — manifest 에는 ssh_user 가 있는데 request 로 옮기지 않으면 **틀린 계정으로 조용히**
    #   위임이 나간다. 서브 위임은 계정이 곧 권한 평면이므로 여기서 fail-closed 한다.
    if transport == "ssh" and (not isinstance(target.get("ssh_user"), str) or not target["ssh_user"]):
        out.append("$.target.ssh_user: required for ssh (계정 미지정 위임은 권한 평면을 바꾼다)")
    if request.get("schema_version") == 2:
        if not isinstance(target.get("work_dir"), str) or not Path(target["work_dir"]).is_absolute():
            out.append("$.target.work_dir: v2 requires an absolute workspace path")
        resume = request.get("resume_session_id")
        binding = request.get("resume_binding")
        selection = {k: request[k] for k in ("sandbox_mode", "permission_profile") if k in request}
        expected = {"provider": request["provider"], "target": target, **selection}
        if resume is not None:
            if not resume.strip() or binding != expected:
                out.append("$.resume_binding: native resume requires the exact provider, target and permission selection")
        elif binding is not None:
            out.append("$.resume_binding: forbidden without resume_session_id")
        profile = request.get("permission_profile")
        if profile and profile != ":danger-full-access":
            out.append("$.permission_profile: must be :danger-full-access for main and sub")
        if role == "sub" and request.get("sandbox_mode") == "workspace-write":
            out.append("$.sandbox_mode: sub writes require :danger-full-access via permission_profile; do not use a legacy override")
    return out


def request_schema_violations(request) -> list[str]:
    """Public transport gate: structural and cross-field request-contract violations.
    Callers (e.g. bootstrap_canary) use this instead of reaching into the private validator."""
    violations = _schema_violations(request, _load_schema(REQUEST_SCHEMA_PATH))
    if not violations:
        violations.extend(_request_contract_violations(request))
        validator = getattr(_load_provider(request["provider"]), "reasoning_violations", None)
        if validator:
            violations.extend(validator(request))
    return violations


def _versioned_result(result: dict) -> dict:
    """Keep v1 envelopes unchanged; v2 always carries explicit observation slots."""
    if result["schema_version"] == 2:
        result.update(raw_exit_code=None, execution_started=None, usage=None,
                      failure_evidence=None, raw_events=[])
    return result


def _invalid_request_result(request) -> dict:
    """Stable, always-schema-conformant fail-closed envelope for a request that did not validate --
    echoes back only the fields that themselves survive validation, falling back to safe defaults
    otherwise, so the result stays valid against agent-control-result.schema.json regardless of how
    badly malformed the input request was."""
    provider = request.get("provider") if isinstance(request, dict) else None
    if provider not in KNOWN_PROVIDERS:
        provider = KNOWN_PROVIDERS[0]
    intent = request.get("intent") if isinstance(request, dict) else None
    if intent not in KNOWN_INTENTS:
        intent = KNOWN_INTENTS[0]
    model_requested = request.get("model") if isinstance(request, dict) else None
    if not isinstance(model_requested, str):
        model_requested = ""
    schema_version = 2 if provider == "codex_cli" else SCHEMA_VERSION_DEFAULT
    return _versioned_result({
        "schema_version": schema_version,
        "provider": provider,
        "intent": intent,
        "status": "invalid_request",
        "exit_code": EXIT_INVALID_REQUEST,
        "model_requested": model_requested,
        "model_used": [],
        "reason_codes": ["REQUEST_SCHEMA_INVALID"],
        "output": None,
        # 릴레이 3필드는 **모르면 null** 이다. 2026-09-04 부터 결과 스키마의 `required` 이며(이 봉투가
        # 그것을 빠뜨려 자체검사가 즉시 잡았다 — 가드가 제 일을 했다), 이 경로에서는 서브에 닿지도
        # 않았으므로 null 이 곧 사실이다. 값이 없다는 것과 키가 없다는 것은 다른 사실이다.
        "session_id": None,
        "num_turns": None,
        "budget_outcome": None,
        # 2026-09-05(F): 소요시간 2필드도 required 다. 닿지 못한 요청에는 잰 시간이 없으므로
        # null 이 곧 사실이다(0 을 적으면 "0ms 만에 끝났다" 는 거짓이 된다).
        "duration_ms": None,
        "duration_api_ms": None,
    })


def _invalid_provider_result(request: dict) -> dict:
    result = _versioned_result({
        "schema_version": request["schema_version"],
        "provider": request["provider"],
        "intent": request["intent"],
        "status": "malformed_output",
        "exit_code": 5,
        "model_requested": request["model"],
        "model_used": [],
        "reason_codes": ["PROVIDER_RESULT_INVALID"],
        "output": None,
        # 동상(2026-09-04): provider 결과가 스키마를 못 지켰을 때의 봉투도 3필드를 갖는다.
        # provider 가 준 값을 여기로 옮기지 않는다 — 그 결과 자체가 무효 판정을 받았기 때문이다.
        "session_id": None,
        "num_turns": None,
        "budget_outcome": None,
        "duration_ms": None,
        "duration_api_ms": None,
    })
    if request.get("capture_native_events") is True:
        result["raw_events"] = []
    return result


def _emit(result: dict) -> None:
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    sys.exit(result["exit_code"])


def _result_contract_violations(result: dict, request: dict) -> list[str]:
    """A valid envelope from another provider/invocation is still the wrong result."""
    out = [f"$.{key}: provider result does not match the dispatched request"
            for key, expected in (("schema_version", request["schema_version"]),
                                  ("provider", request["provider"]),
                                  ("intent", request["intent"]),
                                  ("model_requested", request["model"]))
            if result.get(key) != expected]
    if request["provider"] == "claude_code":
        capture = request.get("capture_native_events") is True
        if capture and "raw_events" not in result:
            out.append("$.raw_events: requested native event evidence is absent")
        elif not capture and "raw_events" in result:
            out.append("$.raw_events: unsolicited native event evidence")
    selection = result.get("reasoning_selection")
    if selection is not None and selection.get("requested") != request.get("reasoning_effort"):
        out.append("$.reasoning_selection: does not match the requested effort")
    return out


def cmd_invoke(args: argparse.Namespace) -> None:
    if bool(getattr(args, "result_file", None)) != bool(getattr(args, "dispatch_id", None)):
        raise SystemExit("--result-file and --dispatch-id must be supplied together")
    request_path = Path(args.request)
    try:
        request = json.loads(request_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, ValueError, RecursionError):
        _emit(_invalid_request_result(None))
        return

    violations = request_schema_violations(request)
    if violations:
        # 2026-09-03(F4): 위반 목록을 계산해 놓고 버렸다 — 사용자는 무엇이 틀렸는지 알 수 없었다.
        #   stdout 은 안정 JSON 계약이므로 stderr 로 낸다.
        for v in violations:
            print(f"[agent-control] REQUEST_SCHEMA_INVALID: {v}", file=sys.stderr)
        _emit(_invalid_request_result(request if isinstance(request, dict) else None))
        return

    provider_module = _load_provider(request["provider"])
    result = provider_module.invoke(request)
    if "reasoning_effort" in request:
        effort = request["reasoning_effort"]
        selection = result.setdefault("reasoning_selection", {
            "requested": effort, "resolved": None if effort == "default" else effort,
            "source": "target-native-policy" if effort == "default" else "request",
            "mode": "native-default" if effort == "default" else "explicit",
            "upstream_observed": None})
        selection["selection_source"] = request.get("reasoning_effort_source", "caller")

    result_schema = _load_schema(RESULT_SCHEMA_PATH)
    result_violations = _schema_violations(result, result_schema)
    if not result_violations:
        result_violations.extend(_result_contract_violations(result, request))
    if result_violations:
        result = _invalid_provider_result(request)
    if getattr(args, "result_file", None):
        persist_outcome(args.result_file, dispatch_id=args.dispatch_id, request=request, result=result)
    _emit(result)


def persist_outcome(path: str, *, dispatch_id: str, request: dict, result: dict) -> None:
    """Publish a unique, fully flushed outcome before stdout for lost-pipe recovery."""
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile("w", dir=destination.parent, prefix=".agent-outcome-",
                                         delete=False, encoding="utf-8") as stream:
            temporary = stream.name
            json.dump({"schema_version": 1, "dispatch_id": dispatch_id,
                       "request": request, "result": result}, stream, ensure_ascii=False, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.link(temporary, destination)  # never overwrite another generation's evidence
        directory = os.open(destination.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if temporary:
            os.unlink(temporary)


def cmd_runners(args: argparse.Namespace) -> None:
    """선택된 provider 어댑터가 아는 **러너 별칭 표**를 중립 JSON 으로 낸다.

    이 파일은 provider 어휘를 알지 못한다 — 어댑터가 소유한 표를 그대로 옮길 뿐이다(라우팅).
    호출자(relay)가 별칭을 스스로 펴면 같은 표가 두 자리에 앉고, 갈라진 쪽이 조용히 늦는다.
    """
    names = KNOWN_PROVIDERS if args.provider == "all" else (args.provider,)
    runners, supports = [], {}
    seen = set()
    for provider_name in names:
        provider_module = _load_provider(provider_name)
        supports[provider_name] = getattr(provider_module, "SUPPORTS", {})
        for name, pair in getattr(provider_module, "RUNNER_ALIASES", {}).items():
            if name in seen:
                raise SystemExit(f"duplicate runner alias: {name}")
            seen.add(name)
            runners.append({"name": name, "provider": provider_name,
                            "backend": pair[0], "model": pair[1]})
    out = {"provider": args.provider, "supports": supports if args.provider == "all"
           else supports[args.provider], "runners": runners}
    print(json.dumps(out, ensure_ascii=False, sort_keys=True, indent=2))
    sys.exit(0)


# =============================================================================
# --self-test (plan_26093022 · 옛 runtime_selftest.py 의 세 테스트를 owner 로 이관)
#   기초층은 이 모듈의 사설 함수를 import 하지 않는다 — `runtime_selftest` 는 이 CLI 를 subprocess 로 부른다.
#   provider 몽키패치는 이 프로세스 안에서만 일어나므로 실제 subprocess 는 한 번도 돌지 않는다.
# =============================================================================
_SELFTEST_MARKER = "# --self-test (plan_26093022"


class AgentControlSelftestFailure(RuntimeError):
    """A required agent-control regression property did not hold."""


def _require(condition: object, message: str) -> None:
    if not condition:
        raise AgentControlSelftestFailure(message)


def _request(transport: str = "local") -> dict:
    target = {"role": "main", "transport": transport, "work_dir": "/tmp/runtime probe"}
    if transport == "ssh":
        target.update({"role": "sub", "host": "192.0.2.10", "ssh_user": "probe"})
    return {
        "schema_version": 1,
        "provider": "claude_code",
        "intent": "probe",
        "task": "read-only runtime probe",
        "model": "sonnet",
        # 2026-09-03(plan_26090317 P1): 이 픽스처는 스키마 required 인 `timeout_seconds` 를 빠뜨리고
        #   있었다 — 실물 request 는 반드시 갖는 필드다. 픽스처가 실물보다 좁으면 그 위의 단언은
        #   실물에서 성립하는 성질을 시험하지 못한다(원격 timeout 래핑이 그 예였다).
        "timeout_seconds": 60,
        "max_turns": 1,
        "capabilities": ["read"],
        "target": target,
    }


def _test_provider_turn_exhaustion_reachable() -> None:
    """`claude -p` 가 **exit 1 + 정상 result JSON** 으로 소진을 알리는 실제 형태를 재현한다.

    2026-09-04(plan_26090317 P4 라이브): 소진 분류 분기가 `returncode != 0` 조기 반환 뒤에 있어
    **한 번도 실행되지 않았다**. 단위 자체검사는 성공 경로만 봤고, 첫 라이브 위임이 알려줬다 —
    원장에 `budget=None`·`turns=None` 만 남아 다음 attempt 예산을 정할 근거가 사라진다.
    그러므로 여기서는 **실측 payload 모양 그대로** 넣고 세 값이 살아 나오는지 본다.
    """
    provider = _load_provider("claude_code")
    payload = {"type": "result", "subtype": "error_max_turns", "is_error": True,
               "num_turns": 26, "session_id": "sess-abc",
               "duration_ms": 812_345, "duration_api_ms": 640_000,
               "errors": ["Reached maximum number of turns (25)"],
               "result": "", "modelUsage": {}}

    class _Completed:
        returncode, stdout, stderr = 1, json.dumps(payload), ""

    real_run = provider.subprocess.run
    provider.subprocess.run = lambda *a, **k: _Completed()
    try:
        req = _request("local")
        req["max_turns"] = 25
        res = provider.invoke(req)
    finally:
        provider.subprocess.run = real_run

    _require(res["budget_outcome"] == "exhausted",
             f"turn exhaustion must be classified as exhausted, got {res.get('budget_outcome')!r} "
             f"-- an unreachable branch leaves the ledger with no basis to size the next attempt")
    _require(res["num_turns"] == 26 and res["session_id"] == "sess-abc",
             f"num_turns/session_id must survive a non-zero exit: {res}")
    _require(res["status"] == "execution_failed",
             "exhaustion is still a failure of that attempt -- it must not read as completed")
    # 2026-09-05(축 F): 결과가 **실제 결과 스키마**를 통과해야 한다. 필드 몇 개만 보던 종전 검사는
    #   새 required 필드가 빠져도 초록이었다(픽스처가 실물보다 좁다).
    _res_schema = _load_schema(RESULT_SCHEMA_PATH)
    _require(not _schema_violations(res, _res_schema),
             f"provider result violates result schema: {_schema_violations(res, _res_schema)}")
    _require(res["duration_ms"] == 812_345 and res["duration_api_ms"] == 640_000,
             f"provider-reported durations must survive a non-zero exit: {res}")

    # 음성대조: payload 가 아예 없는 비-0 종료(전송 실패)는 여전히 NONZERO_EXIT 이고 원장 3필드는 null.
    class _Broken:
        returncode, stdout, stderr = 255, "", "ssh: connect failed"

    provider.subprocess.run = lambda *a, **k: _Broken()
    try:
        res2 = provider.invoke(_request("ssh"))
    finally:
        provider.subprocess.run = real_run
    _require(res2["reason_codes"] == ["NONZERO_EXIT"] and res2["budget_outcome"] is None
             and res2["num_turns"] is None,
             f"a transport failure has no budget story -- it must stay null, got {res2}")
    _require(res2["duration_ms"] is None and res2["duration_api_ms"] is None,
             f"unmeasured durations stay null (0 would read as 'finished instantly'): {res2}")

    # 외생 중단(원격 timeout 124 / SIGTERM 143)은 **예산 사건이 아니다** — 원장이 둘을 갈라야
    # 다음 attempt 의 처방이 뒤집히지 않는다(2026-09-05 · F).
    for _rc in (124, 143):
        class _Killed:
            returncode, stdout, stderr = _rc, "", "Terminated"
        provider.subprocess.run = lambda *a, **k: _Killed()
        try:
            res3 = provider.invoke(_request("ssh"))
        finally:
            provider.subprocess.run = real_run
        _require(res3["budget_outcome"] == "external_interruption",
                 f"rc={_rc} is an external interruption, not a budget outcome: {res3}")


def _test_agent_provider_boundary() -> None:
    request_schema = _load_schema(REQUEST_SCHEMA_PATH)
    result_schema = _load_schema(RESULT_SCHEMA_PATH)
    _require(_schema_violations({}, request_schema),
             "empty agent request unexpectedly passed schema validation")
    invalid = _invalid_request_result({})
    _require(not _schema_violations(invalid, result_schema),
             "invalid-request fail-closed envelope violates result schema")

    provider = _load_provider("claude_code")
    # 경계 검사 대상은 **orchestrator 본문**이다 — 이 파일 아래의 self-test 구획은 어댑터 토큰을 검사
    #   리터럴로 들고 있으므로 구획 표지 앞에서 자른다(표지가 사라지면 검사가 스스로 RED 가 된다).
    _full_source = Path(__file__).read_text(encoding="utf-8")
    _require(_SELFTEST_MARKER in _full_source, "self-test section marker missing -- boundary scan would be void")
    agent_source = _full_source.split(_SELFTEST_MARKER, 1)[0]
    provider_file = getattr(provider, "__file__", None)
    _require(isinstance(provider_file, str), "loaded provider has no source path")
    provider_source = Path(str(provider_file)).read_text(encoding="utf-8")
    # 2026-09-05(G-A1): `--model sonnet` 토큰 강제는 **모델 핀**이었다 — 어댑터 경계는 "claude 문법이
    #   여기에만 산다" 를 지키면 되고, 어느 모델을 부르는지는 요청의 선언이다.
    for token in ("claude -p", "--model", "--output-format json"):
        _require(token not in agent_source, f"provider-specific token leaked into orchestrator: {token}")
        _require(token in provider_source, f"provider adapter lost required CLI token: {token}")

    local_argv = provider.build_argv(_request("local"))
    _require(local_argv[0] == "claude" and "--model" in local_argv,
             f"local provider argv malformed: {local_argv}")
    ssh_argv = provider.build_argv(_request("ssh"))
    # 2026-09-03(F5 · plan_26090317 P1): 위임 전송만 맨 ssh 였다 — 미등록 host key·패스프레이즈에서
    #   ssh 가 /dev/tty 를 읽으며 timeout_seconds(≤3600s)까지 멈추고, 그 뒤에도 **원격 claude 는 살아**
    #   서브 워크스페이스를 계속 편집했다(메인은 이미 실패로 기록한 뒤). 하드닝을 계약으로 고정한다.
    _require(ssh_argv[0] == "ssh", f"SSH provider argv malformed: {ssh_argv}")
    _require("-o" in ssh_argv and "BatchMode=yes" in ssh_argv and "ConnectTimeout=8" in ssh_argv,
             f"SSH delegation must never be able to prompt on a tty: {ssh_argv}")
    _require(ssh_argv[-2] == "probe@192.0.2.10" and ssh_argv[ssh_argv.index("--") + 1] == "probe@192.0.2.10",
             f"SSH destination misplaced: {ssh_argv}")
    remote_shell = shlex.split(ssh_argv[-1])
    _require(remote_shell[:2] == ["bash", "-lc"] and
             remote_shell[2].startswith("cd '/tmp/runtime probe' && "),
             f"SSH work_dir is not safely shell-quoted: {ssh_argv[-1]}")
    # 원격 동반사망: 클라이언트 timeout 만으로는 서브에 고아 에이전트가 남는다.
    _require("timeout " in remote_shell[2] and " claude " in remote_shell[2],
             f"remote command must be wrapped in `timeout` so the sub agent dies with the client: {remote_shell[2]}")
    # 2026-09-05(3-4): 서브 `-p` 릴레이는 재시도 워치독을 켠다. **순서가 계약이다** — env 대입은
    #   `timeout` 앞에 와야 한다(뒤에 두면 timeout 이 `VAR=1` 을 실행 파일로 알고 즉사한다).
    _remote_cmd = remote_shell[2].split("&&", 1)[1].strip()
    _require(_remote_cmd.startswith("CLAUDE_CODE_RETRY_WATCHDOG=1 timeout "),
             f"sub relay must set the retry watchdog before `timeout`: {_remote_cmd[:120]}")
    _require("CLAUDE_CODE_RETRY_WATCHDOG" not in " ".join(local_argv),
             "local transport is the main node's own plane -- the sub relay env must not leak into it")

    # 2026-09-05(G-A1): 모델은 **선언**이다 — 어댑터가 막지 않고, 실제로 돈 모델을 기록한다.
    #   종전 이 자리는 `--model opus` 를 exit 3 으로 차단했고, 그 한 줄 때문에 모델을 바꾸려면
    #   하네스를 고쳐야 했다(모델 과적합의 정면 사례 · audit_26090515 A1).
    _opus_payload = {"type": "result", "subtype": "success", "is_error": False,
                     "num_turns": 2, "session_id": "sess-opus", "result": "done",
                     "duration_ms": 4200, "duration_api_ms": 3900,
                     "modelUsage": {"claude-opus-5": {"canonicalModel": "claude-opus-5"}}}

    class _OpusRun:
        returncode, stdout, stderr = 0, json.dumps(_opus_payload), ""

    _real_run = provider.subprocess.run
    provider.subprocess.run = lambda *a, **k: _OpusRun()
    try:
        declared = _request("local")
        declared["model"] = "opus"
        result = provider.invoke(declared)
    finally:
        provider.subprocess.run = _real_run
    _require(result["status"] == "completed" and result["exit_code"] == 0,
             f"the model is a declaration, not a gate -- opus must run: {result}")
    _require(result["model_used"] == ["claude-opus-5"] and result["model_requested"] == "opus",
             f"the model that actually ran must be recorded: {result}")
    _res_schema2 = _load_schema(RESULT_SCHEMA_PATH)
    _require(not _schema_violations(result, _res_schema2),
             f"non-Sonnet completed result must satisfy the result schema: {result}")

    # metadata 부재는 **기록의 부재**이지 차단 사유가 아니다(model_used=[] 로 남고 결과는 유효하다).
    _bare = dict(_opus_payload); _bare.pop("modelUsage")

    class _BareRun:
        returncode, stdout, stderr = 0, json.dumps(_bare), ""

    provider.subprocess.run = lambda *a, **k: _BareRun()
    try:
        bare_result = provider.invoke(_request("local"))
    finally:
        provider.subprocess.run = _real_run
    _require(bare_result["status"] == "completed" and bare_result["model_used"] == [],
             f"absent model metadata must be recorded as empty, not blocked: {bare_result}")
    _require(not _schema_violations(bare_result, _res_schema2),
             f"metadata-less completed result must satisfy the result schema: {bare_result}")

    # 2026-09-05 회귀: 권한 거부 경로는 거부된 도구 목록을 output 에 실어 돌려주는데, 결과 스키마의
    #   execution_failed 가지가 `output: const null` 이라 그 결과가 **스키마 위반**이 됐고
    #   orchestrator 가 PROVIDER_RESULT_INVALID 봉투로 갈아끼워 진단이 호출자에게 도달하지 못했다.
    _denied = {"type": "result", "subtype": "success", "is_error": False, "num_turns": 1,
               "session_id": "sess-deny", "result": "…", "duration_ms": 10, "duration_api_ms": 5,
               "permission_denials": [{"tool_name": "Write", "tool_input": {"path": "x"}}],
               "modelUsage": {"claude-sonnet-4-5": {"canonicalModel": "claude-sonnet-4-5"}}}

    class _DeniedRun:
        returncode, stdout, stderr = 0, json.dumps(_denied), ""

    provider.subprocess.run = lambda *a, **k: _DeniedRun()
    try:
        denied_result = provider.invoke(_request("local"))
    finally:
        provider.subprocess.run = _real_run
    _require(denied_result["reason_codes"] == ["PERMISSION_DENIED"]
             and "permission_denials" in (denied_result["output"] or ""),
             f"permission denial must carry its diagnosis: {denied_result}")
    _require(not _schema_violations(denied_result, _res_schema2),
             f"permission-denied result must satisfy the result schema: "
             f"{_schema_violations(denied_result, _res_schema2)}")


def _test_runner_ladder_classification() -> None:
    """러너(백엔드×모델) 평면 실패 판정 — **실측 봉투**를 픽스처로 쓴다.

    아래 값은 2026-09-08 에 `claude` 2.1.263 을 실제로 실패시켜 수확한 것이다(추측 문자열 ✗):
      · 정상          rc=0  terminal_reason="completed" api_error_status=null is_error=false
      · 인증실패      rc=1  terminal_reason="api_error" api_error_status=401  is_error=true
      · 미도달        rc=1  terminal_reason="api_error" api_error_status=null is_error=true (ENOTFOUND)
      · 바이너리부재  rc=127 (stdout 없음)
      · ssh 전송실패  rc=255
    ★ 실패 봉투도 `subtype == "success"` 다 — 봉투 형태로는 갈리지 않으므로 판정은
      `terminal_reason` 이라는 **구조 신호**를 읽는다. 픽스처가 실물보다 좁아지지 않도록
      각 항목은 실제로 받은 필드 조합을 그대로 쓴다.
    """
    provider = _load_provider("claude_code")
    cls = provider.classify_runner_failure

    _ok = {"type": "result", "subtype": "success", "is_error": False,
           "terminal_reason": "completed", "api_error_status": None,
           "session_id": "s-ok", "num_turns": 1}
    _require(cls(_ok, 0, "ssh") is None, "정상 봉투가 러너 실패로 판정됐다")

    _401 = {"type": "result", "subtype": "success", "is_error": True,
            "terminal_reason": "api_error", "api_error_status": 401,
            "session_id": "e939c9b7", "num_turns": 1, "duration_api_ms": 0,
            "result": "Failed to authenticate. API Error: 401 ..."}
    _ev = cls(_401, 1, "ssh")
    _require(_ev and _ev["rotate"] is True and _ev["api_error_status"] == 401,
             f"401 인증실패가 회전 대상으로 판정되지 않았다: {_ev}")

    _dns = {"type": "result", "subtype": "success", "is_error": True,
            "terminal_reason": "api_error", "api_error_status": None,
            "session_id": "e14d974b", "num_turns": 1, "duration_api_ms": 0,
            "result": "API Error: Can't reach the API server (ENOTFOUND)"}
    _ev = cls(_dns, 1, "ssh")
    _require(_ev and _ev["rotate"] is True and _ev["api_error_status"] is None,
             f"백엔드 미도달이 회전 대상으로 판정되지 않았다: {_ev}")

    _ev = cls(None, 127, "ssh")
    _require(_ev and _ev["rotate"] is True and _ev["signal"] == "missing_binary",
             "원격 바이너리 부재(rc 127)가 회전 대상이 아니다")

    _require(cls(None, 255, "ssh") is None,
             "★ssh 전송 실패(rc 255)는 회전 대상이 아니다 — 백엔드를 바꿔도 낫지 않는다")

    _400 = dict(_401, api_error_status=400)
    _ev = cls(_400, 1, "ssh")
    _require(_ev and _ev["rotate"] is False,
             "★요청 자체가 거절된 것(400)은 회전해도 같은 거절을 받는다")

    # ★ 음성대조: 서브가 **자기 과업에 실패**한 것은 러너 실패가 아니다. 이 구분이 없으면
    #   빌드 실패 한 번이 사다리를 통째로 태우고 틀린 서사로 HITL 한다.
    _subfail = {"type": "result", "subtype": "success", "is_error": True,
                "terminal_reason": "completed", "session_id": "s-real", "num_turns": 18,
                "duration_api_ms": 40000, "result": "빌드가 실패했다"}
    _require(cls(_subfail, 1, "ssh") is None,
             "★음성대조: 서브 과업 실패가 러너 실패로 접혔다(사다리를 태우는 형태)")

    # 만든 것과 도는 것은 다르다 — invoke() 끝까지 증거가 실제로 **도달하는지** 본다.
    class _Run401:
        returncode, stdout, stderr = 1, json.dumps(_401), ""

    _real_run = provider.subprocess.run
    provider.subprocess.run = lambda *a, **k: _Run401()
    try:
        res = provider.invoke(_request("ssh"))
    finally:
        provider.subprocess.run = _real_run
    _require(res["reason_codes"] == ["RUNNER_UNAVAILABLE"],
             f"401 이 RUNNER_UNAVAILABLE 로 오지 않았다: {res['reason_codes']}")
    _require(res["session_id"] == "e939c9b7" and res["num_turns"] == 1,
             "★러너 실패 결과가 세션·턴을 버렸다(종전 IS_ERROR 경로의 회귀)")
    _require(res["output"] and "runner_unavailable" in res["output"] and "401" in res["output"],
             f"증거가 output 에 실려 오지 않았다: {res['output']!r}")
    _require(res["budget_outcome"] is None,
             "러너 실패에 예산 서사를 붙였다 — 예산을 키워도 죽은 백엔드는 살아나지 않는다")
    _require(not _schema_violations(
        res, _load_schema(RESULT_SCHEMA_PATH)),
        "러너 실패 결과가 결과 스키마를 위반한다")

    # 별칭 표는 어댑터가 소유하고 orchestrator 는 **옮기기만** 한다(사본 ✗).
    _require(set(provider.RUNNER_ALIASES) >= {"opus", "sonnet", "haiku",
                                              "kimi-claude", "minimax-claude", "meta-claude",
                                              "openai-claude"},
             f"러너 별칭 표가 좁다: {sorted(provider.RUNNER_ALIASES)}")
    for _name, (_b, _m) in provider.RUNNER_ALIASES.items():
        _require(_b in provider.BACKEND_TO_BINARY,
                 f"별칭 {_name} 의 backend {_b} 가 바이너리 표에 없다(닫힌 열거가 갈라졌다)")
    # 2026-09-15: 백엔드 어휘는 세 자리(바이너리 표 · 기본모델 표 · 전송 스키마 enum)에 있다. 정적
    #   파일끼리는 한쪽이 다른 쪽을 생성할 수 없으므로 **교차검증**이 차선이다(workflow.md §결정론 규율).
    _enum = set(_load_schema(REQUEST_SCHEMA_PATH)
                ["properties"]["backend"]["enum"])
    _require(_enum == set(provider.BACKEND_TO_BINARY) == set(provider.BACKEND_DEFAULT_MODEL),
             f"백엔드 어휘가 갈라졌다: schema={sorted(_enum)} "
             f"binary={sorted(provider.BACKEND_TO_BINARY)} model={sorted(provider.BACKEND_DEFAULT_MODEL)}")

    for unknown in (418, 499, 999, "429", True, {}, []):
        evidence = cls(dict(_401, api_error_status=unknown), 1, "ssh")
        _require(evidence and evidence["rotate"] is False,
                 f"unknown/wrong-typed HTTP status opened rotation: {unknown!r}")
    absent = dict(_401)
    absent.pop("api_error_status")
    _require(cls(absent, 1, "ssh")["rotate"] is False,
             "missing HTTP status was confused with an observed connection-failure null")
    for filename, transport, rotates in (("/missing/workspace", "local", False),
                                          ("claude", "local", True), ("ssh", "ssh", False)):
        def missing(*args, **kwargs):
            raise FileNotFoundError(2, "fixture missing path", filename)
        provider.subprocess.run = missing
        try:
            value = provider.invoke(_request(transport))
        finally:
            provider.subprocess.run = _real_run
        _require(("RUNNER_UNAVAILABLE" in value["reason_codes"]) == rotates,
                 f"launch path {filename!r} was assigned to the wrong failure plane: {value}")


def _test_codex_provider() -> None:
    import copy
    import tempfile
    import time

    provider = _load_provider("codex_cli")
    request = _request()
    request.update(schema_version=2, provider="codex_cli", model="gpt-6.1-sol",
                   max_turns=None, budget_unit="wall_time", max_dispatches=2,
                   sandbox_mode="read-only")
    _require(not request_schema_violations(request), "legacy v2 wall-time request rejected")
    uncapped = dict(request)
    uncapped.pop("max_dispatches")
    _require(not request_schema_violations(uncapped), "per-invocation v2 request rejected without cumulative count")
    missing_timeout = dict(uncapped)
    missing_timeout.pop("timeout_seconds")
    _require(request_schema_violations(missing_timeout), "v2 request without timeout accepted")
    for key, value in (("max_turns", 1), ("max_dispatches", 0), ("budget_unit", "native_turns")):
        bad = dict(request, **{key: value})
        _require(request_schema_violations(bad), f"unsupported v2 budget accepted: {key}={value!r}")
    for version, name in ((1, "codex_cli"), (2, "claude_code")):
        bad = dict(request, schema_version=version, provider=name)
        _require(request_schema_violations(bad), "provider/version mismatch accepted")
    bad = dict(request, provider="unknown")
    _require(not _schema_violations(_invalid_request_result(bad), _load_schema(RESULT_SCHEMA_PATH)),
             "unknown-provider failure envelope invalid")
    _require(not _schema_violations(_invalid_request_result(request), _load_schema(RESULT_SCHEMA_PATH)),
             "v2 invalid-request failure envelope invalid")
    _require(not _schema_violations(_invalid_provider_result(request), _load_schema(RESULT_SCHEMA_PATH)),
             "v2 invalid-provider failure envelope invalid")

    resume = dict(request, resume_session_id="thread-1")
    _require(request_schema_violations(resume), "unbound native resume accepted")
    binding = {"provider": "codex_cli", "target": dict(request["target"]), "sandbox_mode": "read-only"}
    resume["resume_binding"] = binding
    _require(not request_schema_violations(resume), "bound native resume rejected")
    for change in ({"provider": "claude_code"}, {"sandbox_mode": "workspace-write"},
                   {"target": dict(request["target"], work_dir="/other/workspace")}):
        bad = copy.deepcopy(resume)
        bad["resume_binding"].update(change)
        _require(request_schema_violations(bad), f"mismatched native resume accepted: {change}")
    named = dict(request)
    named.pop("sandbox_mode")
    named["permission_profile"] = ":danger-full-access"
    _require(not request_schema_violations(named), "named-profile v2 request rejected")
    _require(request_schema_violations(dict(named, sandbox_mode="workspace-write")),
             "simultaneous named profile and legacy sandbox accepted")
    _require(request_schema_violations(dict(named, permission_profile="vllm-sub")),
             "retired role-specific profile accepted")
    sub = dict(request, target={"role": "sub", "transport": "ssh", "host": "192.0.2.10",
                               "ssh_user": "probe", "work_dir": "/tmp/sub"})
    _require(request_schema_violations(dict(sub, sandbox_mode="workspace-write")),
             "legacy workspace-write override bypassed sub role protection")
    _require(not request_schema_violations(sub), "read-only sub canary rejected")
    _require(request_schema_violations(dict(request, target=dict(request["target"], work_dir="relative"))),
             "relative workspace was accepted despite workspace lock identity ambiguity")
    missing_workspace = copy.deepcopy(request)
    missing_workspace["target"].pop("work_dir")
    _require(request_schema_violations(missing_workspace), "missing v2 workspace crashed or passed the gate")
    named_argv = provider.build_argv(named)
    _require('--sandbox' not in named_argv and 'default_permissions=":danger-full-access"' in named_argv,
             f"named profile was overridden by legacy sandbox: {named_argv}")
    resume_argv = provider.build_argv(resume)
    _require("resume" in resume_argv and '--last' not in resume_argv
             and 'sandbox_mode="read-only"' in resume_argv,
             "resume must retain explicit sandbox and native ID")
    hostile = dict(request, task="--dangerously-bypass-approvals-and-sandbox; $(touch /tmp/must-not-exist)")
    hostile["target"] = {"role": "sub", "transport": "ssh", "ssh_user": "probe",
                         "host": "192.0.2.10", "work_dir": "/tmp/work space'$(false)"}
    argv = provider.build_argv(hostile)
    script = shlex.split(argv[-1])[2]
    # Exact safe quoted prompt is an operand, not executable shell grammar.
    _require(shlex.quote(hostile["task"]) in script and shlex.quote(hostile["target"]["work_dir"]) in script,
             "SSH prompt/workspace shell quoting changed")
    _require("BatchMode=yes" in argv and "adapter.deadline_reaped" in script
             and "os.killpg" in script and "python3" in script,
             "SSH must have noninteractive identity and remote bounded termination")
    _require(provider._inner_argv(hostile)[-2:] == ["--", hostile["task"]],
             "hostile prompt was not terminated as a positional operand")

    def stream(*events):
        return "".join(json.dumps(e) + "\n" for e in events)
    started = [{"type": "thread.started", "thread_id": "thread-1"}, {"type": "turn.started"}]
    message = {"type": "item.completed", "item": {"type": "agent_message", "text": "final report"}}
    done = {"type": "turn.completed", "usage": {"input_tokens": 12, "output_tokens": 4}}
    good = stream(*started, message, done)
    result_schema = _load_schema(RESULT_SCHEMA_PATH)
    def parsed(raw, rc=0, **kwargs):
        value = provider.parse_output(request, raw, "fixture diagnostic", rc, **kwargs)
        _require(not _schema_violations(value, result_schema),
                 f"Codex result schema violation: {_schema_violations(value, result_schema)}")
        return value
    success = parsed(good)
    _require(_result_contract_violations(dict(success, provider="claude_code"), request),
             "result from another provider was accepted as the dispatched provider")
    _require(success["status"] == "completed" and success["session_id"] == "thread-1"
             and success["usage"] == done["usage"] and success["num_turns"] is None
             and success["model_used"] == [] and success["duration_ms"] is None,
             f"successful native observations were synthesized or lost: {success}")
    for raw in (good + '{"type":', stream(*started, message),
                stream(*started, {"type": "item.completed", "item": {"type": "agent_message", "text": "  "}}, done),
                stream(*started, {"type": "item.completed", "item": {"type": "command_execution", "aggregated_output": "not a final response"}}, done)):
        result = parsed(raw)
        _require(result["status"] == "malformed_output" and result["session_id"] == "thread-1",
                 "truncated/malformed/empty/non-agent output was accepted or lost its session")
    for rc in (0, 1):
        failed = parsed(stream(*started, {"type": "turn.failed", "error": {"message": "quota"}}), rc)
        _require(failed["status"] == "execution_failed" and failed["execution_started"] is True
                 and failed["failure_evidence"]["rotate"] is False,
                 "native failure or late effects permitted automatic replay")
    denied = parsed(stream(*started, {"type": "turn.failed", "error": {"code": "permission_denied"}}))
    _require(denied["reason_codes"] == ["PERMISSION_DENIED"] and not denied["failure_evidence"]["rotate"],
             "permission denial was a rotation signal")
    reported_denial = parsed(stream(*started, {"type": "error", "code": "permission_denied"}, message, done))
    _require("final report" in reported_denial["output"],
             "permission event discarded the final native task report needed by outer completion gates")
    timed = parsed(stream(*started), -15, timed_out=True)
    _require(timed["status"] == "timeout" and timed["session_id"] == "thread-1"
             and timed["budget_outcome"] == "external_interruption", "timeout lost partial observations")
    unknown = parsed("", 1)
    _require(unknown["execution_started"] is None and not unknown["failure_evidence"]["rotate"],
             "unobserved execution was treated as safely replayable")
    # End-to-end process lifetime check, no Codex/model/network. The child ignores TERM;
    # the adapter must kill the entire original process group after its grace period.
    real_inner, real_grace = provider._inner_argv, provider.KILL_GRACE_SECONDS
    try:
        with tempfile.TemporaryDirectory(prefix="codex-provider-selftest-") as temp:
            script_path = Path(temp) / "fixture.py"
            pid_path = Path(temp) / "child.pid"
            script_path.write_text(
                "import subprocess,sys,time,json\n"
                "child=subprocess.Popen([sys.executable,'-c','import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(60)'])\n"
                "open(sys.argv[1],'w').write(str(child.pid))\n"
                "print(json.dumps({'type':'thread.started','thread_id':'timeout-thread'}),flush=True)\n"
                "print(json.dumps({'type':'turn.started'}),flush=True)\n"
                "time.sleep(60)\n", encoding="utf-8")
            provider._inner_argv = lambda _request: [sys.executable, str(script_path), str(pid_path)]
            provider.KILL_GRACE_SECONDS = 0.15
            limited = dict(request, timeout_seconds=1, target={"role": "main", "transport": "local", "work_dir": temp})
            before = time.monotonic()
            value = provider.invoke(limited)
            _require(time.monotonic() - before < 5 and value["status"] == "timeout"
                     and value["session_id"] == "timeout-thread", "deadline did not cleanly terminate fixture")
            child_pid = int(pid_path.read_text())
            stat = Path(f"/proc/{child_pid}/stat")
            # A just-killed child can be a zombie until reaped by its adopted parent.
            try:
                child_state = stat.read_text().split(")", 1)[1].split()[0]
            except (FileNotFoundError, ProcessLookupError):
                child_state = None
            _require(child_state in (None, "Z"),
                     "timeout left a live TERM-ignoring descendant")
            # A separate producer gets a busy result without entering the child.
            entered = Path(temp) / "entered"
            script_path.write_text(
                "import sys,time\n"
                "with open(sys.argv[1],'a') as f: f.write('entered\\n')\n"
                "time.sleep(60)\n", encoding="utf-8")
            provider._inner_argv = lambda _request: [sys.executable, str(script_path), str(entered)]
            locking = dict(limited, timeout_seconds=5)
            first = provider.subprocess.Popen(provider.build_argv(locking), cwd=temp,
                        stdout=provider.subprocess.PIPE, stderr=provider.subprocess.PIPE,
                        start_new_session=True)
            try:
                deadline = time.monotonic() + 2
                while not entered.exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                _require(entered.exists(), "first worker did not acquire the lock")
                busy = provider.invoke(locking)
                _require(busy["failure_evidence"]["signal"] == "worker_busy"
                         and busy["execution_started"] is False
                         and busy["failure_evidence"]["rotate"] is False
                         and entered.read_text() == "entered\n",
                         "workspace lock allowed a duplicate execution or rotated on contention")
            finally:
                provider._stop_process(first)
    finally:
        provider._inner_argv, provider.KILL_GRACE_SECONDS = real_inner, real_grace



def _self_test() -> int:
    _require(REQUEST_SCHEMA_PATH.is_file() and RESULT_SCHEMA_PATH.is_file(),
             f"schema paths must resolve from the repo root (REPO_ROOT={REPO_ROOT})")
    _require(not request_schema_violations(_request("ssh")),
             "the public transport gate must accept a well-formed ssh request")
    _require(request_schema_violations({}), "the public transport gate must reject an empty request")
    for test in (_test_provider_turn_exhaustion_reachable, _test_agent_provider_boundary,
                 _test_runner_ladder_classification, _test_codex_provider):
        test()
    print("[agent_control] self-test PASS (boundary · turn exhaustion · runner ladder · public gate · Codex v2 · worker exclusion)")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="agent_control.py",
        description="provider-neutral agent-control orchestrator (Phase 6, plan_26072506)",
        epilog="exit codes: 0=completed 2=invalid_request "
               "4=execution_failed 5=malformed_output 124=timeout",
    )
    parser.add_argument("--self-test", action="store_true",
                        help="경계·소진·러너 판정·공개 전송 게이트 회귀를 실행한다(네트워크·claude 실행 없음)")
    sub = parser.add_subparsers(dest="cmd")
    invoke_parser = sub.add_parser("invoke", help="invoke a provider against a request JSON file")
    invoke_parser.add_argument("--request", required=True, help="path to an agent-control-request JSON file")
    invoke_parser.add_argument("--result-file", help="optional unique durable dispatch-outcome file")
    invoke_parser.add_argument("--dispatch-id", help="required with --result-file; binds recovery to one dispatch")
    invoke_parser.set_defaults(func=cmd_invoke)
    runners_parser = sub.add_parser("runners", help="provider 어댑터가 아는 러너 별칭 표를 출력한다")
    runners_parser.add_argument("--provider", default=KNOWN_PROVIDERS[0], choices=[*KNOWN_PROVIDERS, "all"])
    runners_parser.set_defaults(func=cmd_runners)
    args = parser.parse_args()
    if args.self_test:
        try:
            sys.exit(_self_test())
        except AgentControlSelftestFailure as exc:
            print(f"[agent_control] self-test FAIL: {exc}", file=sys.stderr)
            sys.exit(1)
    if not getattr(args, "func", None):
        parser.error("a subcommand (invoke | runners) or --self-test is required")
    args.func(args)


if __name__ == "__main__":
    main()
