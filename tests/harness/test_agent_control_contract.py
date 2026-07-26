"""tests/harness/test_agent_control_contract.py -- RED-phase TDD suite for Phase 6
(plan_26072506_하네스_루프_구조개선_Sonnet_E2E.md, section 5.3 + section 6 "Phase 6").

This suite is written BEFORE any of the Phase 6 production files exist. It is the executable
specification the GREEN implementation must satisfy:

    .claude/schemas/agent-control-request.schema.json
    .claude/schemas/agent-control-result.schema.json
    scripts/agent_control.py
    scripts/providers/claude_code.py

Every test in this file is expected to FAIL right now, precisely because one of those four
production files does not exist yet. Missing-file conditions raise a plain AssertionError with an
explicit "missing production file: <path> (Phase 6 not implemented yet)" message (via the small
`_load_schema` / `_require_file` / `_import_claude_provider` helpers below) so the RED run reads as
a clean FAIL list keyed to a specific absent artifact, not a wall of raw tracebacks.

Runner: stdlib `unittest` only (matches tests/harness/test_completion_gate.py's own stated
policy of no new test dependencies). No `jsonschema` import: schema-conformance checks use a small
self-contained Draft-07-subset structural validator (`_schema_violations`), modeled on the same
subset `scripts/completion_gate.py:validate_against_schema` already implements for this repo.

    python3 -m unittest tests.harness.test_agent_control_contract -v
    python3 -m unittest discover -s tests/harness -p 'test_*.py' -v

--------------------------------------------------------------------------------------------
CONTRACT THIS SUITE PINS DOWN (the spec scripts/agent_control.py + scripts/providers/claude_code.py
+ the two schemas must implement to go GREEN)
--------------------------------------------------------------------------------------------

Provider-neutral intent vocabulary (mirrors
`.claude/skills/terraforming_node/references/agent-control-adapter.md` section 1):
    intent in {"delegate", "probe", "bootstrap_canary"}

agent-control-request.schema.json (Draft-07, additionalProperties: false at every object level):
    required top-level: schema_version, provider, intent, model, task, target
    provider:  string, enum ["claude_code"] (the ONLY place the literal token "claude" may appear
               as a schema VALUE; it must never appear as a property/field NAME anywhere in either
               schema -- see TestSchemaFilesAreProviderNeutral)
    intent:    string, enum ["delegate", "probe", "bootstrap_canary"]
    model:     string, minLength 1 -- NOT constrained to "sonnet" by the schema itself (schema
               stays provider/model-family neutral; Sonnet-only enforcement is a RUNTIME safety
               check the adapter performs against the *result* metadata, not a request-schema
               constraint)
    task:      string, minLength 1
    target:    object, required [role, transport], additionalProperties: false
                 role:      enum ["main", "sub"]
                 transport: enum ["local", "ssh"]
                 host:      string (required for transport == "ssh" in practice; not schema-enforced)
                 ssh_user:  string
                 work_dir:  string
    timeout_seconds: integer, minimum 1 (optional)

agent-control-result.schema.json (Draft-07, additionalProperties: false):
    required top-level: schema_version, provider, intent, status, exit_code, model_requested,
                         model_used, reason_codes
    status:       string, enum ["completed", "invalid_request", "model_safety_blocked",
                                 "execution_failed", "malformed_output", "timeout"]
    exit_code:    integer
    model_requested: string
    model_used:   array of string (model ids actually reported by the provider; [] when the
                   provider never got far enough to report any)
    reason_codes: array of string, items enum in STABLE_REASON_CODES (below); [] on success

Exit-code <-> status <-> reason_codes table (single source of truth for this suite):
    0   completed              reason_codes == []
    2   invalid_request        REQUEST_SCHEMA_INVALID
    3   model_safety_blocked   MISSING_MODEL_METADATA | OPUS_FALLBACK | MIXED_MODEL_USAGE
    4   execution_failed       NONZERO_EXIT | IS_ERROR
    5   malformed_output       MALFORMED_JSON
    124 timeout                TIMEOUT

CLI surface:
    python3 scripts/agent_control.py invoke --request <path-to-request.json>
      -> stable JSON (json.dumps(..., sort_keys=True, indent=2), matching scripts/completion_gate.py
         convention) on stdout; process exit code == result["exit_code"]; the CLI is byte-for-byte
         deterministic across repeated invocations of the same request against the same fake
         provider binary (no wall-clock/random fields in the result).
    python3 scripts/agent_control.py --help
      -> exit 0

Claude Code adapter (scripts/providers/claude_code.py) pure-function surface:
    build_argv(request: dict) -> list[str]
        Provider CLI invocation argv for the given request. For transport == "local" this is a
        flat argv list; for transport == "ssh" it is wrapped in an ssh invocation. In both cases
        the flattened, space-joined command text must match `--model\\s+sonnet\\b` whenever
        request["model"] == "sonnet", and `--output-format\\s+json\\b` always (mirrors
        tests/harness/test_skill_contracts.py CLAUDE_ONLY_PATTERNS vocabulary -- this is the ONE
        file in the repo allowed to emit that literal syntax; see
        TestClaudeCommandsIsolatedToAdapter in test_skill_contracts.py).
    invoke(request: dict) -> dict
        Actually runs the provider binary ("claude", resolved via PATH -- tests hermetically
        override this by prepending a fakebin/ directory containing an executable named `claude`
        to PATH, the same convention tests/harness/test_sync_to_sub_gate.py already uses for
        fake ssh/rsync) and returns a result dict shaped per agent-control-result.schema.json.
        Fail-closed (never raises) on: request["target"]["transport"] fake-provider nonzero exit,
        is_error: true in the provider's own JSON, unparseable provider stdout, missing/absent
        "modelUsage" metadata, an all-Opus modelUsage, a mixed Sonnet+Opus modelUsage, and
        subprocess timeout.

Real `claude -p ... --output-format json` emits a top-level "modelUsage" object keyed by full
model-id strings (e.g. "claude-sonnet-4-5-20250929") -> per-model token-usage stats; this is the
wrapper metadata the adapter must inspect to prove Sonnet was actually used (never trust a
"model": "sonnet" request echo -- that only proves what was *asked for*, not what executed).
A model id is classified Sonnet/Opus by case-insensitive substring match on "sonnet"/"opus" (real
ids always embed one of these words).

Fake `claude` executable convention (env-var driven, one generic script reused by every scenario):
    FAKE_CLAUDE_ARGV_LOG       (optional) path; script appends json.dumps(sys.argv) + "\\n"
    FAKE_CLAUDE_SLEEP_SECONDS  (optional, default "0") float; script sleeps this long before exit
    FAKE_CLAUDE_STDOUT_FILE    (required) path; script writes this file's exact bytes to stdout
    FAKE_CLAUDE_EXIT_CODE      (optional, default "0") int; script's own exit code

Providers not in Phase 6 scope (Codex/Pi Agent/OpenCode) must have NO file under scripts/providers/
and must NOT appear in the request schema's "provider" enum.
"""
from __future__ import annotations

import ast
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
REQUEST_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-request.schema.json"
RESULT_SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "agent-control-result.schema.json"
AGENT_CONTROL_SCRIPT = REPO_ROOT / "scripts" / "agent_control.py"
PROVIDERS_DIR = REPO_ROOT / "scripts" / "providers"
CLAUDE_PROVIDER_SCRIPT = PROVIDERS_DIR / "claude_code.py"
GITIGNORE_PATH = REPO_ROOT / ".gitignore"
ADAPTER_REFERENCE_PATH = REPO_ROOT / ".claude" / "skills" / "terraforming_node" / "references" / "agent-control-adapter.md"

MODEL_SONNET_RE = re.compile(r"--model\s+sonnet\b")
OUTPUT_FORMAT_JSON_RE = re.compile(r"--output-format\s+json\b")

STABLE_STATUSES = (
    "completed", "invalid_request", "model_safety_blocked",
    "execution_failed", "malformed_output", "timeout",
)
STABLE_REASON_CODES = (
    "REQUEST_SCHEMA_INVALID", "NONZERO_EXIT", "IS_ERROR", "MALFORMED_JSON",
    "TIMEOUT", "MISSING_MODEL_METADATA", "OPUS_FALLBACK", "MIXED_MODEL_USAGE",
    "REQUESTED_MODEL_NOT_SONNET", "UNEXPECTED_MODEL_USAGE", "PROVIDER_RESULT_INVALID",
    "PERMISSION_DENIED",
)
EXIT_SUCCESS = 0
EXIT_INVALID_REQUEST = 2
EXIT_MODEL_SAFETY_BLOCKED = 3
EXIT_EXECUTION_FAILED = 4
EXIT_MALFORMED_OUTPUT = 5
EXIT_TIMEOUT = 124

BANNED_FIELD_NAME_TOKENS = (
    "claude", "sonnet", "opus", "permission_mode", "permission-mode",
    "output_format", "output-format", "bypasspermissions", "acceptedits",
    "dangerously_skip_permissions", "dangerously-skip-permissions",
)

MAIN_WORK_DIR = "/home/cona/ws_docker/easy_vllm_simulator"
SUB_WORK_DIR = "/home/subuser/easy_vllm_simulator"
SUB_HOST = "sub-node.internal"
SUB_SSH_USER = "subuser"

SONNET_MODEL_ID = "claude-sonnet-4-5-20250929"
OPUS_MODEL_ID = "claude-opus-4-1-20250805"

# Fake `claude` CLI test double -- see module docstring "Fake claude executable convention".
# Raw string: every `\n`/`\\n` below must survive verbatim into the written-out script file, to be
# interpreted by *that* script's own Python interpreter, not by this file's.
FAKE_CLAUDE_SCRIPT = r"""#!/usr/bin/env python3
# hermetic test double for the `claude` CLI -- records argv, optionally sleeps, then emits fixed
# stdout content and exits with a fixed code. NEVER touches the network.
import json
import os
import sys
import time

argv_log = os.environ.get("FAKE_CLAUDE_ARGV_LOG")
if argv_log:
    with open(argv_log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(sys.argv) + "\n")

sleep_seconds = float(os.environ.get("FAKE_CLAUDE_SLEEP_SECONDS", "0"))
if sleep_seconds:
    time.sleep(sleep_seconds)

stdout_file = os.environ["FAKE_CLAUDE_STDOUT_FILE"]
with open(stdout_file, "rb") as fh:
    sys.stdout.buffer.write(fh.read())

sys.exit(int(os.environ.get("FAKE_CLAUDE_EXIT_CODE", "0")))
"""


# ==================================================================================================
# helpers -- schema loading, a small Draft-07-subset structural validator, provider import,
# request/target fixtures, fake-`claude`-binary plumbing, agent_control.py CLI subprocess plumbing.
# ==================================================================================================

def _require_file(path: Path, what: str) -> None:
    if not path.is_file():
        raise AssertionError(f"missing production {what}: {path} (Phase 6 / plan_26072506 not implemented yet)")


def _load_schema(path: Path) -> dict:
    _require_file(path, "schema file")
    return json.loads(path.read_text(encoding="utf-8"))


def _import_claude_provider():
    _require_file(CLAUDE_PROVIDER_SCRIPT, "provider adapter")
    spec = importlib.util.spec_from_file_location("claude_code_provider_under_test", CLAUDE_PROVIDER_SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


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


def _schema_violations(instance, schema: dict, path: str = "$") -> list:
    """Small Draft-07-subset structural validator (type/const/enum/minLength/minimum/minItems/
    items/required/properties/additionalProperties(bool only)). Deliberately does not implement
    $ref/allOf/if-then-else -- the Phase 6 schemas this suite specifies are flat and do not need
    them (unlike completion-manifest.schema.json's cross-field invariants)."""
    out = []
    if "oneOf" in schema:
        matches = sum(not _schema_violations(instance, branch, path) for branch in schema["oneOf"])
        if matches != 1:
            out.append(f"{path}: expected exactly one oneOf branch, matched {matches}")
    if "type" in schema:
        allowed = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(_json_type_matches(instance, t) for t in allowed):
            out.append(f"{path}: expected type {allowed}, got {type(instance).__name__} ({instance!r})")
            return out
    if "const" in schema and instance != schema["const"]:
        out.append(f"{path}: expected const {schema['const']!r}, got {instance!r}")
    if "enum" in schema and instance not in schema["enum"]:
        out.append(f"{path}: {instance!r} not in enum {schema['enum']!r}")
    if isinstance(instance, str) and "minLength" in schema and len(instance) < schema["minLength"]:
        out.append(f"{path}: length {len(instance)} below minLength {schema['minLength']}")
    if isinstance(instance, (int, float)) and not isinstance(instance, bool) and "minimum" in schema \
            and instance < schema["minimum"]:
        out.append(f"{path}: {instance!r} below minimum {schema['minimum']}")
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            out.append(f"{path}: {len(instance)} items, below minItems {schema['minItems']}")
        if "items" in schema:
            for i, item in enumerate(instance):
                out.extend(_schema_violations(item, schema["items"], f"{path}[{i}]"))
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


def _iter_schema_field_names(node):
    """Yields every key that appears as a field NAME (i.e. as a key inside a `properties` map,
    recursively through nested properties/items/definitions) -- used for the provider-neutrality
    sweep. Deliberately does NOT walk enum/const/default VALUES: "claude_code" is allowed to
    appear as the value of the `provider` enum; it must never appear as a field name."""
    if not isinstance(node, dict):
        return
    props = node.get("properties")
    if isinstance(props, dict):
        for key, subschema in props.items():
            yield key
            yield from _iter_schema_field_names(subschema)
    items = node.get("items")
    if isinstance(items, dict):
        yield from _iter_schema_field_names(items)
    defs = node.get("definitions") or node.get("$defs")
    if isinstance(defs, dict):
        for subschema in defs.values():
            yield from _iter_schema_field_names(subschema)


def _target(role: str) -> dict:
    if role == "main":
        return {"role": "main", "transport": "local", "work_dir": MAIN_WORK_DIR}
    if role == "sub":
        return {"role": "sub", "transport": "ssh", "host": SUB_HOST,
                "ssh_user": SUB_SSH_USER, "work_dir": SUB_WORK_DIR}
    raise ValueError(role)


def _request(role: str, *, model: str = "sonnet", intent: str = "delegate", task: str = "ping",
             timeout_seconds: int = 30, provider: str = "claude_code") -> dict:
    return {
        "schema_version": 1,
        "provider": provider,
        "intent": intent,
        "model": model,
        "task": task,
        "target": _target(role),
        "timeout_seconds": timeout_seconds,
        "max_turns": 8,
        "capabilities": ["read"],
    }


def _write_fake_claude(bindir: Path) -> Path:
    bindir.mkdir(parents=True, exist_ok=True)
    path = bindir / "claude"
    path.write_text(FAKE_CLAUDE_SCRIPT, encoding="utf-8")
    path.chmod(0o755)
    return path


def _claude_stdout_payload(*, is_error: bool = False, model_usage=None, result_text: str = "ok") -> str:
    payload = {
        "type": "result",
        "subtype": "error" if is_error else "success",
        "is_error": is_error,
        "result": result_text,
        "session_id": "fake-session-0001",
    }
    if model_usage is not None:
        normalized = {}
        for model_id, metadata in model_usage.items():
            metadata = dict(metadata) if isinstance(metadata, dict) else metadata
            if isinstance(metadata, dict):
                metadata.setdefault("canonicalModel", str(model_id))
            normalized[model_id] = metadata
        payload["modelUsage"] = normalized
    return json.dumps(payload)


def _security_interpreter_flags() -> list[str]:
    flags = []
    if sys.flags.isolated:
        flags.append("-I")
    elif sys.flags.no_site:
        flags.append("-S")
    if sys.flags.optimize:
        flags.append("-" + "O" * sys.flags.optimize)
    if sys.flags.dont_write_bytecode:
        flags.append("-B")
    return flags


def _run_agent_control(request_path: Path, *, env: dict, timeout: float = 15) -> subprocess.CompletedProcess:
    _require_file(AGENT_CONTROL_SCRIPT, "CLI")
    argv = [sys.executable, *_security_interpreter_flags(), str(AGENT_CONTROL_SCRIPT),
            "invoke", "--request", str(request_path)]
    return subprocess.run(argv, cwd=str(REPO_ROOT), env=env, capture_output=True, text=True, timeout=timeout)


class _AgentControlCliCase(unittest.TestCase):
    """Shared scaffolding for tests that drive scripts/agent_control.py as a real subprocess
    against the hermetic fake `claude` binary on PATH (tests/harness/test_sync_to_sub_gate.py's
    fakebin/PATH-prepend convention, reused here for the same reason: never touch the network)."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.bindir = self.tmp / "fakebin"
        _write_fake_claude(self.bindir)
        self.argv_log = self.tmp / "argv.log"
        self.stdout_file = self.tmp / "claude_stdout.json"

    def tearDown(self):
        self._tmp.cleanup()

    def _env(self, *, exit_code: int = 0, sleep_seconds: float = 0, argv_log: bool = True) -> dict:
        env = dict(os.environ)
        env["PATH"] = f"{self.bindir}{os.pathsep}{env.get('PATH', '')}"
        env["FAKE_CLAUDE_STDOUT_FILE"] = str(self.stdout_file)
        env["FAKE_CLAUDE_EXIT_CODE"] = str(exit_code)
        env["FAKE_CLAUDE_SLEEP_SECONDS"] = str(sleep_seconds)
        if argv_log:
            env["FAKE_CLAUDE_ARGV_LOG"] = str(self.argv_log)
        return env

    def _write_request(self, request: dict, name: str = "request.json") -> Path:
        path = self.tmp / name
        path.write_text(json.dumps(request), encoding="utf-8")
        return path

    def _invoke(self, request: dict, *, stdout_payload: str, provider_exit_code: int = 0,
                sleep_seconds: float = 0, timeout: float = 15) -> tuple:
        """Writes stdout_payload for the fake `claude` binary to emit, runs agent_control.py
        against `request`, and returns (CompletedProcess, parsed_result_dict)."""
        self.stdout_file.write_text(stdout_payload, encoding="utf-8")
        req_path = self._write_request(request)
        env = self._env(exit_code=provider_exit_code, sleep_seconds=sleep_seconds)
        cp = _run_agent_control(req_path, env=env, timeout=timeout)
        try:
            result = json.loads(cp.stdout)
        except json.JSONDecodeError as e:
            self.fail(
                f"scripts/agent_control.py did not print valid JSON to stdout "
                f"(exit={cp.returncode}): {e}\nstdout={cp.stdout!r}\nstderr={cp.stderr!r}"
            )
        return cp, result

    def _assert_fail_closed(self, cp: subprocess.CompletedProcess, result: dict, *,
                             exit_code: int, status: str, reason_code: str) -> None:
        self.assertEqual(exit_code, cp.returncode, msg=f"result={result}\nstderr={cp.stderr}")
        self.assertEqual(exit_code, result.get("exit_code"), msg=result)
        self.assertEqual(status, result.get("status"), msg=result)
        self.assertIn(reason_code, result.get("reason_codes", []), msg=result)
        violations = _schema_violations(result, _load_schema(RESULT_SCHEMA_PATH))
        self.assertEqual([], violations, msg=violations)


# ==================================================================================================
# A -- schema files: exist, are valid Draft-07, are provider-neutral
# ==================================================================================================
class TestSchemaFilesAreProviderNeutral(unittest.TestCase):
    def test_result_schema_closes_status_exit_reason_output_invariants(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        contradictory = {
            "schema_version": 1, "provider": "claude_code", "intent": "delegate",
            "status": "completed", "exit_code": 2, "model_requested": "sonnet",
            "model_used": [], "reason_codes": ["REQUEST_SCHEMA_INVALID"], "output": None,
        }
        self.assertTrue(_schema_violations(contradictory, schema))
        self.assertEqual(6, len(schema.get("oneOf", [])))
    def test_agent_control_schemas_are_allowlisted_for_clean_index_exports(self):
        rules = GITIGNORE_PATH.read_text(encoding="utf-8").splitlines()
        for name in ("agent-control-request.schema.json", "agent-control-result.schema.json"):
            self.assertIn("!.claude/schemas/" + name, rules)

    def test_request_schema_file_exists(self):
        _require_file(REQUEST_SCHEMA_PATH, "schema file")

    def test_result_schema_file_exists(self):
        _require_file(RESULT_SCHEMA_PATH, "schema file")

    def test_request_schema_declares_draft07(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        self.assertTrue(schema.get("$schema", "").startswith("http://json-schema.org/draft-07"),
                         msg=schema.get("$schema"))

    def test_result_schema_declares_draft07(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        self.assertTrue(schema.get("$schema", "").startswith("http://json-schema.org/draft-07"),
                         msg=schema.get("$schema"))

    def test_request_schema_required_top_level_fields(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        self.assertEqual(
            {"schema_version", "provider", "intent", "model", "task", "target",
             "timeout_seconds", "max_turns", "capabilities"},
            set(schema.get("required", [])),
        )

    def test_request_schema_version_is_exactly_one(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        self.assertEqual(1, schema["properties"]["schema_version"].get("const"))

    def test_result_schema_required_top_level_fields(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        self.assertEqual(
            {"schema_version", "provider", "intent", "status", "exit_code",
             "model_requested", "model_used", "reason_codes", "output"},
            set(schema.get("required", [])),
        )

    def test_request_schema_forbids_unknown_top_level_properties(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        self.assertFalse(schema.get("additionalProperties", True))

    def test_result_schema_forbids_unknown_top_level_properties(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        self.assertFalse(schema.get("additionalProperties", True))

    def test_request_schema_provider_enum_is_claude_code_only(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        provider_schema = schema.get("properties", {}).get("provider", {})
        self.assertEqual(["claude_code"], provider_schema.get("enum"))

    def test_request_schema_intent_enum_matches_adapter_intents(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        intent_schema = schema.get("properties", {}).get("intent", {})
        self.assertEqual({"delegate", "probe", "bootstrap_canary"}, set(intent_schema.get("enum", [])))

    def test_result_schema_status_enum_is_stable_vocabulary(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        status_schema = schema.get("properties", {}).get("status", {})
        self.assertEqual(set(STABLE_STATUSES), set(status_schema.get("enum", [])))

    def test_result_schema_reason_codes_items_enum_is_stable_vocabulary(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        rc_schema = schema.get("properties", {}).get("reason_codes", {})
        items = rc_schema.get("items", {})
        self.assertEqual(set(STABLE_REASON_CODES), set(items.get("enum", [])))

    def test_no_claude_specific_field_names_in_request_schema(self):
        schema = _load_schema(REQUEST_SCHEMA_PATH)
        offending = [k for k in _iter_schema_field_names(schema)
                     if any(tok in k.lower() for tok in BANNED_FIELD_NAME_TOKENS)]
        self.assertEqual([], offending,
                          msg=f"provider-neutrality violation -- Claude-specific field name(s): {offending}")

    def test_no_claude_specific_field_names_in_result_schema(self):
        schema = _load_schema(RESULT_SCHEMA_PATH)
        offending = [k for k in _iter_schema_field_names(schema)
                     if any(tok in k.lower() for tok in BANNED_FIELD_NAME_TOKENS)]
        self.assertEqual([], offending,
                          msg=f"provider-neutrality violation -- Claude-specific field name(s): {offending}")


# ==================================================================================================
# B -- request payloads: main and sub targets both explicitly declare model=sonnet, both validate
# ==================================================================================================
class TestRequestPayloadsMainAndSubExplicitSonnet(unittest.TestCase):
    def test_main_target_request_declares_model_sonnet_and_validates(self):
        request = _request("main")
        self.assertEqual("sonnet", request["model"])
        self.assertEqual("main", request["target"]["role"])
        violations = _schema_violations(request, _load_schema(REQUEST_SCHEMA_PATH))
        self.assertEqual([], violations, msg=violations)

    def test_sub_target_request_declares_model_sonnet_and_validates(self):
        request = _request("sub")
        self.assertEqual("sonnet", request["model"])
        self.assertEqual("sub", request["target"]["role"])
        violations = _schema_violations(request, _load_schema(REQUEST_SCHEMA_PATH))
        self.assertEqual([], violations, msg=violations)

    def test_request_missing_model_field_is_schema_invalid(self):
        request = _request("main")
        del request["model"]
        violations = _schema_violations(request, _load_schema(REQUEST_SCHEMA_PATH))
        self.assertTrue(any("model" in v for v in violations), msg=violations)

    def test_request_with_unknown_provider_is_schema_invalid(self):
        request = _request("main", provider="codex")
        violations = _schema_violations(request, _load_schema(REQUEST_SCHEMA_PATH))
        self.assertTrue(any("provider" in v for v in violations), msg=violations)


# ==================================================================================================
# C -- scripts/providers/claude_code.py pure-function argv contract (no subprocess execution)
# ==================================================================================================
class TestClaudeCodeAdapterArgvBuildsModelSonnet(unittest.TestCase):
    def test_adapter_maps_generic_bounds_and_capabilities_to_claude_cli(self):
        mod = _import_claude_provider()
        for role in ("main", "sub"):
            with self.subTest(role=role):
                joined = " ".join(mod.build_argv(_request(role)))
                self.assertRegex(joined, r"--max-turns\s+8\b")
                self.assertRegex(joined, r"--allowedTools\s+Read\b")

    def setUp(self):
        self.provider = _import_claude_provider()

    def test_main_target_local_argv_contains_model_sonnet(self):
        argv = self.provider.build_argv(_request("main"))
        combined = " ".join(str(x) for x in argv)
        self.assertRegex(combined, MODEL_SONNET_RE)

    def test_sub_target_ssh_argv_contains_model_sonnet(self):
        argv = self.provider.build_argv(_request("sub"))
        combined = " ".join(str(x) for x in argv)
        self.assertRegex(combined, MODEL_SONNET_RE)

    def test_main_and_sub_argv_both_request_output_format_json(self):
        for role in ("main", "sub"):
            with self.subTest(role=role):
                argv = self.provider.build_argv(_request(role))
                combined = " ".join(str(x) for x in argv)
                self.assertRegex(combined, OUTPUT_FORMAT_JSON_RE)

    def test_sub_target_argv_is_ssh_wrapped(self):
        argv = self.provider.build_argv(_request("sub"))
        self.assertEqual("ssh", argv[0], msg=argv)
        self.assertEqual("--", argv[1], "ssh destination must be after option terminator")
        self.assertIn("bash -lc", " ".join(argv), "sub adapter must load login-shell PATH")

    def test_main_target_argv_is_not_ssh_wrapped(self):
        argv = self.provider.build_argv(_request("main"))
        self.assertNotEqual("ssh", argv[0], msg=argv)

    def test_argv_never_contains_bypass_permissions(self):
        for role in ("main", "sub"):
            with self.subTest(role=role):
                argv = self.provider.build_argv(_request(role))
                combined = " ".join(str(x) for x in argv)
                self.assertNotIn("bypassPermissions", combined)
                self.assertNotIn("--dangerously-skip-permissions", combined)


# ==================================================================================================
# D -- real subprocess round trip through scripts/agent_control.py against a hermetic fake `claude`
# executable on PATH (the "실제 CLI path 검증" requirement)
# ==================================================================================================
class TestFakeClaudeSubprocessRoundTrip(_AgentControlCliCase):
    def test_deeply_nested_or_oversized_integer_json_is_structured(self):
        malformed_documents = [
            ("[" * 2000 + "0" + "]" * 2000).encode("utf-8"),
            ("1" * 5000).encode("ascii"),
        ]
        for index, malformed in enumerate(malformed_documents):
            with self.subTest(source="request", index=index):
                request_path = self.tmp / f"resource-limit-request-{index}.json"
                request_path.write_bytes(malformed)
                cp = _run_agent_control(request_path, env=self._env())
                result = json.loads(cp.stdout)
                self._assert_fail_closed(cp, result, exit_code=EXIT_INVALID_REQUEST,
                                         status="invalid_request", reason_code="REQUEST_SCHEMA_INVALID")

            with self.subTest(source="provider", index=index):
                self.stdout_file.write_bytes(malformed)
                request_path = self._write_request(_request("main"), f"valid-provider-request-{index}.json")
                cp = _run_agent_control(request_path, env=self._env())
                result = json.loads(cp.stdout)
                self._assert_fail_closed(cp, result, exit_code=EXIT_MALFORMED_OUTPUT,
                                         status="malformed_output", reason_code="MALFORMED_JSON")

    def test_invalid_utf8_request_and_provider_stdout_are_structured(self):
        request_path = self.tmp / "invalid-utf8-request.json"
        request_path.write_bytes(b"\xff")
        cp = _run_agent_control(request_path, env=self._env())
        result = json.loads(cp.stdout)
        self._assert_fail_closed(cp, result, exit_code=EXIT_INVALID_REQUEST,
                                 status="invalid_request", reason_code="REQUEST_SCHEMA_INVALID")

        self.stdout_file.write_bytes(b"\xff")
        request_path = self._write_request(_request("main"), "valid-request.json")
        cp = _run_agent_control(request_path, env=self._env())
        result = json.loads(cp.stdout)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MALFORMED_OUTPUT,
                                 status="malformed_output", reason_code="MALFORMED_JSON")

    def test_permission_denials_fail_closed(self):
        payload = {
            "type": "result", "subtype": "success", "is_error": False, "result": "ok",
            "modelUsage": {SONNET_MODEL_ID: {"canonicalModel": SONNET_MODEL_ID}},
            "permission_denials": [{"tool_name": "Read"}],
        }
        cp, result = self._invoke(_request("main"), stdout_payload=json.dumps(payload))
        self._assert_fail_closed(cp, result, exit_code=EXIT_EXECUTION_FAILED,
                                 status="execution_failed", reason_code="PERMISSION_DENIED")

    def test_model_usage_canonical_identity_is_required_and_must_match(self):
        for metadata in ({}, {"canonicalModel": "claude-opus-4"}, {"canonicalModel": 7},
                         {"canonicalModel": "claude-sonnet-different"}):
            payload = {
                "type": "result", "subtype": "success", "is_error": False, "result": "ok",
                "modelUsage": {SONNET_MODEL_ID: metadata},
            }
            with self.subTest(metadata=metadata):
                cp, result = self._invoke(_request("main"), stdout_payload=json.dumps(payload))
                self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                         status="model_safety_blocked",
                                         reason_code="UNEXPECTED_MODEL_USAGE")

    def test_missing_bounds_or_invalid_capabilities_are_invalid_request(self):
        invalid_requests = []
        for missing in ("timeout_seconds", "max_turns", "capabilities"):
            request = _request("main")
            request.pop(missing)
            invalid_requests.append(request)
        for capabilities in ([], ["bash"], ["read", "read"]):
            request = _request("main")
            request["capabilities"] = capabilities
            invalid_requests.append(request)
        for request in invalid_requests:
            with self.subTest(request=request):
                req_path = self._write_request(request)
                cp = _run_agent_control(req_path, env=self._env())
                self.assertEqual(EXIT_INVALID_REQUEST, cp.returncode,
                                 msg="stdout=%r stderr=%r" % (cp.stdout, cp.stderr))
                self.assertFalse(self.argv_log.exists())

    def test_non_sonnet_request_is_blocked_before_provider_execution(self):
        req_path = self._write_request(_request("main", model="opus"))
        cp = _run_agent_control(req_path, env=self._env())
        result = json.loads(cp.stdout)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                 status="model_safety_blocked",
                                 reason_code="REQUESTED_MODEL_NOT_SONNET")
        self.assertFalse(self.argv_log.exists())

    def test_target_role_transport_and_endpoint_contract_fails_before_provider(self):
        invalid_requests = []
        for role, transport in (("main", "ssh"), ("sub", "local")):
            request = _request(role)
            request["target"]["transport"] = transport
            invalid_requests.append(request)
        for missing in ("host", "work_dir"):
            request = _request("sub")
            request["target"].pop(missing)
            invalid_requests.append(request)
        request = _request("main")
        request["target"].pop("work_dir")
        invalid_requests.append(request)
        for field in ("host", "ssh_user", "work_dir"):
            request = _request("sub")
            request["target"][field] = ""
            invalid_requests.append(request)

        for index, request in enumerate(invalid_requests):
            with self.subTest(index=index, target=request["target"]):
                req_path = self._write_request(request, "invalid-%d.json" % index)
                cp = _run_agent_control(req_path, env=self._env())
                self.assertEqual(EXIT_INVALID_REQUEST, cp.returncode,
                                 msg="stdout=%r stderr=%r" % (cp.stdout, cp.stderr))
                result = json.loads(cp.stdout)
                self._assert_fail_closed_generic(cp, result)
                self.assertFalse(self.argv_log.exists(), "provider must not run for invalid target")

    def test_sonnet_only_model_usage_completes_and_matches_result_schema(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 12, "outputTokens": 34}})
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self.assertEqual(EXIT_SUCCESS, cp.returncode, msg=f"result={result}\nstderr={cp.stderr}")
        self.assertEqual("completed", result.get("status"), msg=result)
        self.assertEqual([], result.get("reason_codes"), msg=result)
        self.assertEqual("sonnet", result.get("model_requested"), msg=result)
        self.assertIn(SONNET_MODEL_ID, result.get("model_used", []), msg=result)
        self.assertEqual("ok", result.get("output"), msg=result)
        violations = _schema_violations(result, _load_schema(RESULT_SCHEMA_PATH))
        self.assertEqual([], violations, msg=violations)

    def test_fake_executable_actually_invoked_with_model_sonnet_argv(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        self._invoke(_request("main"), stdout_payload=payload)
        self.assertTrue(self.argv_log.is_file(),
                         msg="fake `claude` binary was never actually exec'd -- adapter did not "
                             "take the real subprocess path")
        recorded = json.loads(self.argv_log.read_text(encoding="utf-8").strip().splitlines()[-1])
        self.assertTrue(recorded[0].endswith("claude"), msg=recorded)
        combined = " ".join(recorded)
        self.assertRegex(combined, MODEL_SONNET_RE)

    def test_missing_model_usage_metadata_fails_closed(self):
        payload = _claude_stdout_payload(model_usage=None)
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                  status="model_safety_blocked", reason_code="MISSING_MODEL_METADATA")

    def test_empty_model_usage_metadata_fails_closed(self):
        payload = _claude_stdout_payload(model_usage={})
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                  status="model_safety_blocked", reason_code="MISSING_MODEL_METADATA")

    def test_opus_only_model_usage_fails_closed_as_opus_fallback(self):
        payload = _claude_stdout_payload(model_usage={OPUS_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                  status="model_safety_blocked", reason_code="OPUS_FALLBACK")

    def test_mixed_sonnet_and_opus_model_usage_fails_closed(self):
        payload = _claude_stdout_payload(model_usage={
            SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1},
            OPUS_MODEL_ID: {"inputTokens": 1, "outputTokens": 1},
        })
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                  status="model_safety_blocked", reason_code="MIXED_MODEL_USAGE")

    def test_unknown_or_mixed_unknown_model_usage_fails_closed(self):
        for usage in (
            {"custom-model": {}},
            {SONNET_MODEL_ID: {}, "custom-model": {}},
            {7: {}},
        ):
            with self.subTest(usage=usage):
                payload = _claude_stdout_payload(model_usage=usage)
                cp, result = self._invoke(_request("main"), stdout_payload=payload)
                self._assert_fail_closed(cp, result, exit_code=EXIT_MODEL_SAFETY_BLOCKED,
                                         status="model_safety_blocked",
                                         reason_code="UNEXPECTED_MODEL_USAGE")

    def test_provider_wrapper_requires_exact_success_shape_and_string_result(self):
        base = {"is_error": False, "subtype": "success",
                "modelUsage": {SONNET_MODEL_ID: {}}}
        malformed = [
            {k: v for k, v in dict(base, result="ok").items() if k != "is_error"},
            dict(base, result=None),
            dict(base, subtype="error", result="ok"),
        ]
        for payload in malformed:
            with self.subTest(payload=payload):
                cp, result = self._invoke(_request("main"), stdout_payload=json.dumps(payload))
                self._assert_fail_closed(cp, result, exit_code=EXIT_MALFORMED_OUTPUT,
                                         status="malformed_output",
                                         reason_code="PROVIDER_RESULT_INVALID")

    def test_provider_wrapper_requires_result_discriminator_and_well_typed_denials(self):
        base = {
            "type": "result", "subtype": "success", "is_error": False, "result": "ok",
            "modelUsage": {SONNET_MODEL_ID: {"canonicalModel": SONNET_MODEL_ID}},
        }
        malformed = [
            {key: value for key, value in base.items() if key != "type"},
            dict(base, type="assistant"),
            dict(base, permission_denials=None),
        ]
        for payload in malformed:
            with self.subTest(payload=payload):
                cp, result = self._invoke(_request("main"), stdout_payload=json.dumps(payload))
                self._assert_fail_closed(cp, result, exit_code=EXIT_MALFORMED_OUTPUT,
                                         status="malformed_output",
                                         reason_code="PROVIDER_RESULT_INVALID")

    def test_is_error_true_fails_closed(self):
        payload = _claude_stdout_payload(
            is_error=True, model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self._assert_fail_closed(cp, result, exit_code=EXIT_EXECUTION_FAILED,
                                  status="execution_failed", reason_code="IS_ERROR")

    def test_nonzero_provider_exit_fails_closed(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        cp, result = self._invoke(_request("main"), stdout_payload=payload, provider_exit_code=1)
        self._assert_fail_closed(cp, result, exit_code=EXIT_EXECUTION_FAILED,
                                  status="execution_failed", reason_code="NONZERO_EXIT")

    def test_malformed_json_stdout_fails_closed(self):
        cp, result = self._invoke(_request("main"), stdout_payload="not-json-at-all{{{")
        self._assert_fail_closed(cp, result, exit_code=EXIT_MALFORMED_OUTPUT,
                                  status="malformed_output", reason_code="MALFORMED_JSON")

    def test_timeout_fails_closed(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        request = _request("main", timeout_seconds=1)
        cp, result = self._invoke(request, stdout_payload=payload, sleep_seconds=5, timeout=15)
        self._assert_fail_closed(cp, result, exit_code=EXIT_TIMEOUT, status="timeout", reason_code="TIMEOUT")

    def test_invalid_request_schema_violation_is_exit2(self):
        request = _request("main")
        del request["task"]
        req_path = self._write_request(request)
        env = self._env()
        cp = _run_agent_control(req_path, env=env)
        try:
            result = json.loads(cp.stdout)
        except json.JSONDecodeError as e:
            self.fail(f"no JSON on stdout (exit={cp.returncode}): {e}\nstderr={cp.stderr!r}")
        self._assert_fail_closed_generic(cp, result)

    def _assert_fail_closed_generic(self, cp, result):
        self.assertEqual(EXIT_INVALID_REQUEST, cp.returncode, msg=result)
        self.assertEqual("invalid_request", result.get("status"), msg=result)
        self.assertIn("REQUEST_SCHEMA_INVALID", result.get("reason_codes", []), msg=result)


# ==================================================================================================
# E -- result schema conformance + CLI JSON stability/determinism
# ==================================================================================================
class TestResultSchemaAndCliStableJson(_AgentControlCliCase):
    def test_repeated_invocation_of_same_request_yields_byte_identical_stdout(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        req_path = self._write_request(_request("main"))
        self.stdout_file.write_text(payload, encoding="utf-8")
        first = _run_agent_control(req_path, env=self._env())
        second = _run_agent_control(req_path, env=self._env())
        self.assertEqual(first.stdout, second.stdout, msg=(first.stdout, second.stdout))

    def test_result_top_level_keys_are_sorted(self):
        payload = _claude_stdout_payload(model_usage={SONNET_MODEL_ID: {"inputTokens": 1, "outputTokens": 1}})
        cp, result = self._invoke(_request("main"), stdout_payload=payload)
        self.assertEqual(sorted(result.keys()), list(result.keys()), msg=result)

    def test_all_observed_reason_codes_are_from_stable_vocabulary(self):
        scenarios = [
            (_claude_stdout_payload(model_usage=None), {}),
            (_claude_stdout_payload(model_usage={OPUS_MODEL_ID: {}}), {}),
            (_claude_stdout_payload(is_error=True, model_usage={SONNET_MODEL_ID: {}}), {}),
        ]
        for payload, extra_kwargs in scenarios:
            with self.subTest(payload=payload):
                cp, result = self._invoke(_request("main"), stdout_payload=payload, **extra_kwargs)
                for code in result.get("reason_codes", []):
                    self.assertIn(code, STABLE_REASON_CODES, msg=result)


# ==================================================================================================
# F -- provider placeholders: Codex/Pi Agent/OpenCode must not exist; Claude Code is the only one
# ==================================================================================================
class TestOnlyClaudeCodeAdapterExistsNoPlaceholders(unittest.TestCase):
    def test_progressive_disclosure_reference_routes_to_bounded_orchestrator(self):
        text = ADAPTER_REFERENCE_PATH.read_text(encoding="utf-8")
        self.assertIn("scripts/agent_control.py invoke", text)
        self.assertIn("--model sonnet", text)
        self.assertIn("capabilities", text)
        self.assertIn("max_turns", text)

    PLACEHOLDER_NAMES = ("codex.py", "pi_agent.py", "pi.py", "opencode.py", "gemini.py", "gpt.py")

    def test_claude_code_adapter_file_exists(self):
        _require_file(CLAUDE_PROVIDER_SCRIPT, "provider adapter")

    def test_named_provider_placeholders_are_absent(self):
        for name in self.PLACEHOLDER_NAMES:
            with self.subTest(name=name):
                self.assertFalse((PROVIDERS_DIR / name).exists(), msg=f"{name} must not exist -- Phase 6 scope is Claude Code only")

    def test_providers_dir_contains_only_claude_code_and_optional_init(self):
        if not PROVIDERS_DIR.is_dir():
            self.fail(f"missing production directory: {PROVIDERS_DIR} (Phase 6 not implemented yet)")
        py_files = {p.name for p in PROVIDERS_DIR.glob("*.py")}
        self.assertEqual({"claude_code.py"}, py_files - {"__init__.py"})


# ==================================================================================================
# G -- CLI surface sanity
# ==================================================================================================
class TestCliSurfaceSanity(unittest.TestCase):
    def test_production_orchestrator_contains_no_optimized_away_asserts(self):
        tree = ast.parse(AGENT_CONTROL_SCRIPT.read_text(encoding="utf-8"))
        self.assertFalse(any(isinstance(node, ast.Assert) for node in ast.walk(tree)))

    def test_child_python_security_flags_are_propagated(self):
        expected = []
        if sys.flags.isolated:
            expected.append("-I")
        elif sys.flags.no_site:
            expected.append("-S")
        if sys.flags.optimize:
            expected.append("-" + "O" * sys.flags.optimize)
        if sys.flags.dont_write_bytecode:
            expected.append("-B")
        self.assertEqual(expected, _security_interpreter_flags())

    def test_help_flag_exits_zero(self):
        _require_file(AGENT_CONTROL_SCRIPT, "CLI")
        result = subprocess.run([sys.executable, str(AGENT_CONTROL_SCRIPT), "--help"],
                                 cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=10)
        self.assertEqual(0, result.returncode, msg=result.stderr)

    def test_missing_request_flag_is_nonzero_exit(self):
        _require_file(AGENT_CONTROL_SCRIPT, "CLI")
        result = subprocess.run([sys.executable, str(AGENT_CONTROL_SCRIPT), "invoke"],
                                 cwd=str(REPO_ROOT), capture_output=True, text=True, timeout=10)
        self.assertNotEqual(0, result.returncode)


if __name__ == "__main__":
    unittest.main()
