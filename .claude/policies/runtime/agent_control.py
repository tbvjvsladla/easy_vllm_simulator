#!/usr/bin/env python3
"""Constitution-owned provider-neutral agent-control orchestrator (Phase 6, plan_26072506).

Reads a request JSON conforming to `.claude/schemas/agent-control-request.schema.json`, dispatches
it to the sibling `providers/` adapter namespace (Phase 6 scope: `claude_code` only
-- see the sibling `providers/claude_code.py`), and prints a stable
(`json.dumps(..., sort_keys=True, indent=2)`) result JSON conforming to
`.claude/schemas/agent-control-result.schema.json` to stdout, exiting with `result["exit_code"]`.

This file stays provider-neutral: it never embeds Claude-specific argv/metadata parsing (that
lives entirely in the sibling provider adapter).  The production enforcement is
`.claude/policies/runtime/runtime_selftest.py::_test_agent_provider_boundary`.

Exit-code <-> status <-> reason_codes table (single source of truth):
    0   completed              reason_codes == []
    2   invalid_request        REQUEST_SCHEMA_INVALID
    3   model_safety_blocked   model identity/request mismatch
    4   execution_failed       NONZERO_EXIT | IS_ERROR
    5   malformed_output       MALFORMED_JSON
    124 timeout                TIMEOUT

Usage:
    python3 .claude/policies/runtime/agent_control.py invoke --request <path-to-request.json>
    python3 .claude/policies/runtime/agent_control.py --help
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

SCHEMA_VERSION_DEFAULT = 1
RUNTIME_DIR = Path(__file__).resolve().parent
REPO_ROOT = Path(__file__).resolve().parents[3]
REQUEST_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-request.schema.json"
RESULT_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-result.schema.json"
PROVIDERS_DIR = RUNTIME_DIR / "providers"

KNOWN_INTENTS = ("delegate", "probe", "bootstrap_canary")
KNOWN_PROVIDERS = ("claude_code",)

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
    if (role, transport) not in (("main", "local"), ("sub", "ssh")):
        out.append("$.target: role/transport mismatch")
    if not isinstance(target.get("work_dir"), str) or not target["work_dir"]:
        out.append("$.target.work_dir: required")
    if transport == "ssh" and (not isinstance(target.get("host"), str) or not target["host"]):
        out.append("$.target.host: required for ssh")
    return out


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
    schema_version = request.get("schema_version") if isinstance(request, dict) else None
    if schema_version != SCHEMA_VERSION_DEFAULT or isinstance(schema_version, bool):
        schema_version = SCHEMA_VERSION_DEFAULT
    return {
        "schema_version": schema_version,
        "provider": provider,
        "intent": intent,
        "status": "invalid_request",
        "exit_code": EXIT_INVALID_REQUEST,
        "model_requested": model_requested,
        "model_used": [],
        "reason_codes": ["REQUEST_SCHEMA_INVALID"],
        "output": None,
    }


def _invalid_provider_result(request: dict) -> dict:
    return {
        "schema_version": SCHEMA_VERSION_DEFAULT,
        "provider": request["provider"],
        "intent": request["intent"],
        "status": "malformed_output",
        "exit_code": 5,
        "model_requested": request["model"],
        "model_used": [],
        "reason_codes": ["PROVIDER_RESULT_INVALID"],
        "output": None,
    }


def _emit(result: dict) -> None:
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    sys.exit(result["exit_code"])


def cmd_invoke(args: argparse.Namespace) -> None:
    request_path = Path(args.request)
    try:
        request = json.loads(request_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, ValueError, RecursionError):
        _emit(_invalid_request_result(None))
        return

    request_schema = _load_schema(REQUEST_SCHEMA_PATH)
    violations = _schema_violations(request, request_schema)
    if not violations:
        violations.extend(_request_contract_violations(request))
    if violations:
        _emit(_invalid_request_result(request if isinstance(request, dict) else None))
        return

    provider_module = _load_provider(request["provider"])
    result = provider_module.invoke(request)

    result_schema = _load_schema(RESULT_SCHEMA_PATH)
    result_violations = _schema_violations(result, result_schema)
    if result_violations:
        result = _invalid_provider_result(request)
    _emit(result)


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="agent_control.py",
        description="provider-neutral agent-control orchestrator (Phase 6, plan_26072506)",
        epilog="exit codes: 0=completed 2=invalid_request 3=model_safety_blocked "
               "4=execution_failed 5=malformed_output 124=timeout",
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    invoke_parser = sub.add_parser("invoke", help="invoke a provider against a request JSON file")
    invoke_parser.add_argument("--request", required=True, help="path to an agent-control-request JSON file")
    invoke_parser.set_defaults(func=cmd_invoke)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
