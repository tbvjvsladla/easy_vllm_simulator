#!/usr/bin/env python3
"""Owner-local deterministic regression probe for constitution runtime modules.

This is production distribution evidence, not a development-only root test suite.  It exercises
security-critical negative paths under the active interpreter, including ``python -O`` and
``python -S`` when invoked by ``harness_verify.py`` / ``verify_distribution.py``.
"""
from __future__ import annotations

import ast
import contextlib
import io
import json
import os
import shlex
import tempfile
from pathlib import Path

import agent_control
import completion_gate
import evidence_publisher
import policy_registry

RUNTIME_DIR = Path(__file__).resolve().parent
PREDICATES_DIR = RUNTIME_DIR.parent / "predicates"
CLAUDE_DIR = RUNTIME_DIR.parents[1]


class RuntimeSelftestFailure(RuntimeError):
    """A required production regression property did not hold."""


def _require(condition: object, message: str) -> None:
    if not condition:
        raise RuntimeSelftestFailure(message)


def _test_no_production_asserts() -> None:
    offenders: list[str] = []
    for path in CLAUDE_DIR.rglob("*.py"):
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        offenders.extend(f"{path.relative_to(RUNTIME_DIR.parents[2])}:{node.lineno}"
                         for node in ast.walk(tree) if isinstance(node, ast.Assert))
    _require(not offenders, f"bare assert is optimization-unsafe: {offenders}")


def _test_completion_gate() -> None:
    work_enum = completion_gate.WORK_SCHEMA["definitions"]["executionApproval"]["properties"]["allowed_actions"]["items"]["enum"]
    side_enum = [value for value in completion_gate.SIDE_EFFECT_SCHEMA["properties"]["action"]["enum"]
                 if value is not None]
    expected_actions = set(completion_gate.ALLOWED_ACTIONS)
    _require(set(work_enum) == expected_actions and set(side_enum) == expected_actions,
             f"side-effect action enum drift: runtime={sorted(expected_actions)}, "
             f"work={sorted(work_enum)}, authorization={sorted(side_enum)}")
    try:
        completion_gate._resolve_ref("https://invalid.example/ref", {})
    except ValueError:
        pass
    else:
        raise RuntimeSelftestFailure("completion gate accepted a non-local $ref")

    try:
        completion_gate._emit_with_schema(
            {}, 0, {"title": "runtime-selftest", "type": "object", "required": ["must_exist"]}
        )
    except RuntimeError:
        pass
    else:
        raise RuntimeSelftestFailure("completion gate emitted schema-invalid own output")

    with tempfile.TemporaryDirectory(prefix="completion-runtime-selftest.") as td:
        root = Path(td)
        manifest_dir = root / "docs" / "_evidence"
        manifest_dir.mkdir(parents=True)
        status, parts = completion_gate._lexical_components(
            manifest_dir, root, "../../../outside.txt"
        )
        _require(status == "repo_escape" and parts is None,
                 f"lexical repo escape was not rejected: {status}, {parts}")

        target = root / "target.txt"
        target.write_text("evidence", encoding="utf-8")
        (root / "link.txt").symlink_to(target)
        root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            status, fd = completion_gate._walk_nofollow(root_fd, ["link.txt"])
            if fd is not None:
                os.close(fd)
            _require(status == "symlink", f"symlink evidence was not rejected: {status}")

            unreadable, broken, invalid = completion_gate.check_required_local_links(
                root_fd, root, root, b"[required evidence](missing.md)"
            )
            _require(not unreadable and broken == ["missing.md"] and not invalid,
                     f"broken Markdown link was not detected: {unreadable}, {broken}, {invalid}")
        finally:
            os.close(root_fd)


def _test_policy_and_evidence_lifecycle() -> None:
    _require(policy_registry.scan_policy_citations("policy:SELFTEST_OK") == [(1, "SELFTEST_OK")],
             "policy citation broad scanner failed its canonical positive")
    _require(policy_registry.scan_policy_citations("xpolicy:NOT_A_CITATION") == [],
             "policy citation scanner accepted an embedded token")
    with contextlib.redirect_stdout(io.StringIO()):
        _require(policy_registry._self_test() == 0, "policy registry self-test did not return 0")
        evidence_publisher._self_test()


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
        "max_turns": 1,
        "capabilities": ["read"],
        "target": target,
    }


def _test_agent_provider_boundary() -> None:
    request_schema = agent_control._load_schema(agent_control.REQUEST_SCHEMA_PATH)
    result_schema = agent_control._load_schema(agent_control.RESULT_SCHEMA_PATH)
    _require(agent_control._schema_violations({}, request_schema),
             "empty agent request unexpectedly passed schema validation")
    invalid = agent_control._invalid_request_result({})
    _require(not agent_control._schema_violations(invalid, result_schema),
             "invalid-request fail-closed envelope violates result schema")

    provider = agent_control._load_provider("claude_code")
    agent_source = Path(agent_control.__file__).read_text(encoding="utf-8")
    provider_file = getattr(provider, "__file__", None)
    _require(isinstance(provider_file, str), "loaded provider has no source path")
    provider_source = Path(str(provider_file)).read_text(encoding="utf-8")
    for token in ("claude -p", "--model sonnet", "--output-format json"):
        _require(token not in agent_source, f"provider-specific token leaked into orchestrator: {token}")
        _require(token in provider_source, f"provider adapter lost required CLI token: {token}")

    local_argv = provider.build_argv(_request("local"))
    _require(local_argv[0] == "claude" and "--model" in local_argv,
             f"local provider argv malformed: {local_argv}")
    ssh_argv = provider.build_argv(_request("ssh"))
    _require(ssh_argv[:2] == ["ssh", "--"] and ssh_argv[2] == "probe@192.0.2.10",
             f"SSH provider argv malformed: {ssh_argv}")
    remote_shell = shlex.split(ssh_argv[3])
    _require(remote_shell[:2] == ["bash", "-lc"] and
             remote_shell[2].startswith("cd '/tmp/runtime probe' && claude "),
             f"SSH work_dir is not safely shell-quoted: {ssh_argv[3]}")

    blocked = _request("local")
    blocked["model"] = "opus"
    result = provider.invoke(blocked)
    _require(result["status"] == "model_safety_blocked" and result["exit_code"] == 3,
             f"non-Sonnet request was not blocked before execution: {result}")


def main() -> int:
    _test_no_production_asserts()
    _test_completion_gate()
    _test_policy_and_evidence_lifecycle()
    _test_agent_provider_boundary()
    print("[runtime_selftest] PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
