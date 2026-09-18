#!/usr/bin/env python3
"""Agent Card structural/capability metadata validator.

Endpoint authentication belongs to the Terraform-provisioned OpenSSH transport.  The card
advertises interfaces, capabilities, skills, and the derived node-role extension; it is neither
a credential nor an action-authorization token.
"""
from __future__ import annotations

import argparse
import json
import sys
from urllib.parse import urlparse

EXIT_OK = 0
EXIT_USAGE = 2
EXIT_CONTRACT = 5

CARD_REQUIRED = ("name", "description", "supportedInterfaces", "version", "capabilities",
                 "defaultInputModes", "defaultOutputModes", "skills")
CARD_OPTIONAL = ("provider", "documentationUrl", "securitySchemes", "securityRequirements", "iconUrl")
CARD_FIELDS = frozenset(CARD_REQUIRED + CARD_OPTIONAL)
INTERFACE_REQUIRED = ("url", "protocolBinding", "protocolVersion")
INTERFACE_FIELDS = frozenset(INTERFACE_REQUIRED + ("tenant",))
SKILL_REQUIRED = ("id", "name", "description", "tags", "inputModes", "outputModes")
SKILL_FIELDS = frozenset(SKILL_REQUIRED + ("examples", "securityRequirements", "extendedDocumentation"))
CAPABILITY_FIELDS = frozenset(("streaming", "pushNotifications", "extendedAgentCard", "extensions"))
EXTENSION_FIELDS = frozenset(("uri", "description", "required", "params"))
PROVIDER_REQUIRED = ("url", "organization")
STANDARD_BINDINGS = frozenset(("JSONRPC", "GRPC", "HTTP+JSON"))
NODE_ROLE_EXT_URI = "urn:easy-vllm:ext:node-role:v1"
NODE_ROLE_PARAMS = ("topology", "sub_mode", "sub_mode_source", "rank", "rank_source",
                    "identity_authority", "delivery_plane", "tool_plane", "tool_plane_source")
EXPECTED_SKILL_IDS = ("inspect", "config", "build", "serve", "bench", "publish")


class ContractViolation(ValueError):
    pass


def _is_uri(value) -> bool:
    if not isinstance(value, str) or not value:
        return False
    p = urlparse(value)
    return bool(p.scheme and (p.netloc or p.path))


def _no_floats(value, path="$") -> None:
    if isinstance(value, float):
        raise ContractViolation(f"{path}: float 금지(결정론 metadata는 정수/문자열로 표현)")
    if isinstance(value, dict):
        for key, child in value.items():
            _no_floats(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _no_floats(child, f"{path}[{index}]")


def validate_card(card: dict) -> list[str]:
    """Validate the A2A fields and this project's node-role capability extension."""
    violations: list[str] = []
    if not isinstance(card, dict):
        return ["카드가 JSON 객체가 아니다"]
    try:
        _no_floats(card)
    except ContractViolation as exc:
        violations.append(str(exc))
    unknown = sorted(key for key in card if key not in CARD_FIELDS)
    if unknown:
        violations.append(f"표준 밖 최상위 키 {unknown} — 고유 항목은 capabilities.extensions[] 에만 둔다")
    for key in CARD_REQUIRED:
        if key not in card:
            violations.append(f"필수 필드 누락: {key}")
    for key in ("name", "description", "version"):
        if key in card and (not isinstance(card[key], str) or (key != "description" and not card[key].strip())):
            violations.append(f"{key}: 비어 있지 않은 문자열이어야 한다")

    interfaces = card.get("supportedInterfaces")
    if interfaces is not None:
        if not isinstance(interfaces, list) or not interfaces:
            violations.append("supportedInterfaces: 비어 있지 않은 배열")
        else:
            for index, interface in enumerate(interfaces):
                if not isinstance(interface, dict):
                    violations.append(f"supportedInterfaces[{index}]: 객체가 아니다")
                    continue
                for key in INTERFACE_REQUIRED:
                    if not isinstance(interface.get(key), str) or not interface.get(key):
                        violations.append(f"supportedInterfaces[{index}].{key}: 필수 문자열")
                bad = sorted(key for key in interface if key not in INTERFACE_FIELDS)
                if bad:
                    violations.append(f"supportedInterfaces[{index}]: 표준 밖 키 {bad}")
                binding = interface.get("protocolBinding")
                if isinstance(binding, str) and binding not in STANDARD_BINDINGS and not _is_uri(binding):
                    violations.append(f"supportedInterfaces[{index}].protocolBinding={binding!r}: 표준 binding 또는 URI여야 한다")

    for key in ("defaultInputModes", "defaultOutputModes"):
        value = card.get(key)
        if value is not None and (not isinstance(value, list) or not value
                                  or not all(isinstance(item, str) and item for item in value)):
            violations.append(f"{key}: 비어 있지 않은 문자열 배열")

    capabilities = card.get("capabilities")
    if capabilities is not None:
        if not isinstance(capabilities, dict):
            violations.append("capabilities: 객체여야 한다")
        else:
            bad = sorted(key for key in capabilities if key not in CAPABILITY_FIELDS)
            if bad:
                violations.append(f"capabilities: 표준 밖 키 {bad}")
            for key in ("streaming", "pushNotifications", "extendedAgentCard"):
                if key in capabilities and not isinstance(capabilities[key], bool):
                    violations.append(f"capabilities.{key}: bool")
            extensions = capabilities.get("extensions", [])
            if not isinstance(extensions, list):
                violations.append("capabilities.extensions: 배열")
                extensions = []
            role_extensions = []
            for index, extension in enumerate(extensions):
                if not isinstance(extension, dict):
                    violations.append(f"capabilities.extensions[{index}]: 객체가 아니다")
                    continue
                bad = sorted(key for key in extension if key not in EXTENSION_FIELDS)
                if bad:
                    violations.append(f"capabilities.extensions[{index}]: 표준 밖 키 {bad}")
                if not _is_uri(extension.get("uri")):
                    violations.append(f"capabilities.extensions[{index}].uri: URI여야 한다")
                if "required" in extension and not isinstance(extension["required"], bool):
                    violations.append(f"capabilities.extensions[{index}].required: bool")
                if "params" in extension and not isinstance(extension["params"], dict):
                    violations.append(f"capabilities.extensions[{index}].params: 객체")
                if extension.get("uri") == NODE_ROLE_EXT_URI:
                    role_extensions.append(extension)
            if len(role_extensions) != 1:
                violations.append(f"노드 역할 확장 {NODE_ROLE_EXT_URI} 는 정확히 1개여야 한다(현재 {len(role_extensions)})")
            else:
                extension = role_extensions[0]
                params = extension.get("params") or {}
                missing = [key for key in NODE_ROLE_PARAMS if key not in params]
                extra = sorted(key for key in params if key not in NODE_ROLE_PARAMS)
                if missing:
                    violations.append(f"node-role 확장 params 누락: {missing}")
                if extra:
                    violations.append(f"node-role 확장 params 표준 밖 키: {extra}")
                if "rank" in params and not (params["rank"] is None or isinstance(params["rank"], int)):
                    violations.append("node-role.rank: 정수 또는 null(single)")
                if "tool_plane" in params and not (isinstance(params["tool_plane"], list)
                                                     and all(isinstance(item, str) for item in params["tool_plane"])):
                    violations.append("node-role.tool_plane: 문자열 배열")
                for key in ("topology", "sub_mode", "sub_mode_source", "rank_source",
                            "identity_authority", "delivery_plane", "tool_plane_source"):
                    if key in params and (not isinstance(params[key], str) or not params[key]):
                        violations.append(f"node-role.{key}: 비어 있지 않은 문자열")
                if extension.get("required") is not True:
                    violations.append("node-role 확장은 required: true")

    skills = card.get("skills")
    if skills is not None:
        if not isinstance(skills, list) or not skills:
            violations.append("skills: 비어 있지 않은 배열")
        else:
            ids = []
            for index, skill in enumerate(skills):
                if not isinstance(skill, dict):
                    violations.append(f"skills[{index}]: 객체가 아니다")
                    continue
                for key in SKILL_REQUIRED:
                    if key not in skill:
                        violations.append(f"skills[{index}].{key}: 필수")
                bad = sorted(key for key in skill if key not in SKILL_FIELDS)
                if bad:
                    violations.append(f"skills[{index}]: 표준 밖 키 {bad}")
                if "tags" in skill and (not isinstance(skill["tags"], list) or not skill["tags"]):
                    violations.append(f"skills[{index}].tags: 비어 있지 않은 배열")
                ids.append(skill.get("id"))
            if len(set(ids)) != len(ids):
                violations.append("skills[].id 중복")

    provider = card.get("provider")
    if provider is not None and (not isinstance(provider, dict)
                                 or any(not isinstance(provider.get(key), str) for key in PROVIDER_REQUIRED)):
        violations.append("provider: {url, organization} 둘 다 필수 문자열")
    return violations


def require_skills(card: dict, expected=EXPECTED_SKILL_IDS) -> list[str]:
    have = {skill.get("id") for skill in card.get("skills") or [] if isinstance(skill, dict)}
    return [f"skill 누락: {key}" for key in expected if key not in have]


def _fixture_card() -> dict:
    return {
        "name": "fixture", "description": "fixture", "version": "2.0.0",
        "supportedInterfaces": [{"url": "ssh://sub", "protocolBinding": "urn:test:ssh", "protocolVersion": "1.0"}],
        "capabilities": {"streaming": False, "extensions": [{
            "uri": NODE_ROLE_EXT_URI, "required": True,
            "params": {"topology": "single", "sub_mode": "a2a-agent",
                       "sub_mode_source": "derived-from-topology", "rank": None,
                       "rank_source": "not-applicable:single", "identity_authority": "ssh-public-key",
                       "delivery_plane": "dormant", "tool_plane": [], "tool_plane_source": "fixture"}}]},
        "defaultInputModes": ["text/plain"], "defaultOutputModes": ["application/json"],
        "skills": [{"id": key, "name": key, "description": key, "tags": [key],
                    "inputModes": ["text/plain"], "outputModes": ["application/json"]}
                   for key in EXPECTED_SKILL_IDS],
    }


def _self_test() -> int:
    import copy
    ok = True

    def check(name, condition):
        nonlocal ok
        print(f"  [{'PASS' if condition else 'FAIL'}] {name}")
        ok &= bool(condition)

    card = _fixture_card()
    check("metadata card contract", validate_card(card) == [] and require_skills(card) == [])
    bad = copy.deepcopy(card); bad["node_identity"] = {"role": "sub"}
    check("unknown top-level identity field rejected", any("표준 밖 최상위" in item for item in validate_card(bad)))
    bad = copy.deepcopy(card); bad["supportedInterfaces"][0]["protocolBinding"] = "ssh-claude-p"
    check("non-URI custom binding rejected", any("URI" in item for item in validate_card(bad)))
    bad = copy.deepcopy(card); del bad["capabilities"]["extensions"][0]["params"]["tool_plane"]
    check("missing role capability rejected", any("params 누락" in item for item in validate_card(bad)))
    bad = copy.deepcopy(card); bad["version"] = 1.5
    check("float rejected", any("float" in item for item in validate_card(bad)))
    bad = copy.deepcopy(card); bad["skills"] = bad["skills"][:4]
    check("missing skills reported", require_skills(bad) == ["skill 누락: bench", "skill 누락: publish"])
    bad = copy.deepcopy(card); bad["signatures"] = [{"protected": "x", "signature": "y"}]
    check("credential field rejected", any("표준 밖 최상위" in item for item in validate_card(bad)))
    print(f"self-test: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description="Agent Card capability metadata validator")
    parser.add_argument("--self-test", action="store_true")
    subparsers = parser.add_subparsers(dest="cmd")
    command = subparsers.add_parser("validate")
    command.add_argument("--card", required=True)
    command.add_argument("--require-skills", action="store_true", default=True)
    args = parser.parse_args(argv)
    if args.self_test:
        return _self_test()
    if args.cmd != "validate":
        parser.print_help()
        return EXIT_USAGE
    with open(args.card, encoding="utf-8") as stream:
        card = json.load(stream)
    violations = validate_card(card)
    if args.require_skills:
        violations += require_skills(card)
    if violations:
        print("[agent-card] 계약 위반:\n  - " + "\n  - ".join(violations), file=sys.stderr)
        return EXIT_CONTRACT
    print(f"[agent-card] OK: metadata fields {len(card)} · skills {len(card['skills'])} · unsigned")
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
