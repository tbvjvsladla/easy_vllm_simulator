"""tests/harness/test_schema_meta_validation.py -- TDD for scripts/policy_registry.py's schema
TRUST layer (plan_26072506 Phase 4 cycle4 remediation of subagent-summary-1 finding 4).

The gap this closes: `--schema {}` disabled schema validation outright (every keyword-presence
check in the generic engine is `if "keyword" in schema: ...`, so an empty dict trivially satisfies
all of them and returns zero violations for ANY instance); and a malformed-but-truthy schema
document (`{"type": 1}`, `{"required": 5}`, `{"properties": ["not", "dict"]}`, a malformed
`definitions`, wrong-typed `items`, or a bare `oneOf`/`anyOf`) crashed the generic engine with an
uncaught Python traceback (process exit 1), contradicting this module's own documented exit-2
invalid-input contract.

The fix: `schema_meta_violations` validates the schema DOCUMENT itself against a strict
supported-shape contract before the generic engine ever touches it, and `schema_violations` wraps
the generic engine call in a catch-all exception handler as defense in depth.

Runner: stdlib `unittest`.
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
POLICY_REGISTRY = SCRIPTS_DIR / "policy_registry.py"
SCHEMA_PATH = REPO_ROOT / ".claude" / "schemas" / "policy-registry.schema.json"
REGISTRY_PATH = REPO_ROOT / ".claude" / "policies" / "registry.yaml"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402


def _load_json(path: Path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


REAL_SCHEMA = _load_json(SCHEMA_PATH)


class TestSchemaMetaViolationsUnit(unittest.TestCase):
    def test_canonical_schema_passes_with_zero_meta_violations(self):
        self.assertEqual(pr.schema_meta_violations(REAL_SCHEMA), [])

    def test_empty_dict_root_is_rejected(self):
        violations = pr.schema_meta_violations({})
        self.assertEqual([v.reason_code for v in violations], ["SCHEMA_META_EMPTY_ROOT"])

    def test_non_dict_root_is_rejected(self):
        for bad in ([], "not-a-schema", 5, None, True):
            with self.subTest(bad=bad):
                violations = pr.schema_meta_violations(bad)
                self.assertEqual([v.reason_code for v in violations], ["SCHEMA_META_NOT_OBJECT"])

    def test_wrong_type_keyword_value_is_rejected(self):
        violations = pr.schema_meta_violations({"type": 1})
        self.assertIn("SCHEMA_META_BAD_TYPE_KEYWORD", [v.reason_code for v in violations])

    def test_unsupported_type_value_is_rejected(self):
        violations = pr.schema_meta_violations({"type": "banana"})
        self.assertIn("SCHEMA_META_UNSUPPORTED_TYPE_VALUE", [v.reason_code for v in violations])

    def test_wrong_type_required_is_rejected(self):
        violations = pr.schema_meta_violations({"type": "object", "required": 5})
        self.assertIn("SCHEMA_META_BAD_REQUIRED", [v.reason_code for v in violations])

    def test_wrong_type_properties_is_rejected(self):
        violations = pr.schema_meta_violations({"type": "object", "properties": ["not", "dict"]})
        self.assertIn("SCHEMA_META_BAD_PROPERTIES", [v.reason_code for v in violations])

    def test_wrong_type_definitions_is_rejected(self):
        violations = pr.schema_meta_violations({"type": "object", "definitions": "not-a-dict"})
        self.assertIn("SCHEMA_META_BAD_DEFINITIONS", [v.reason_code for v in violations])

    def test_wrong_type_items_is_rejected(self):
        violations = pr.schema_meta_violations({"type": "array", "items": "not-a-schema"})
        self.assertIn("SCHEMA_META_NOT_OBJECT", [v.reason_code for v in violations])

    def test_oneof_keyword_is_rejected_as_unsupported(self):
        violations = pr.schema_meta_violations({"type": "object", "oneOf": [{"type": "string"}]})
        self.assertIn("SCHEMA_META_UNSUPPORTED_KEYWORD", [v.reason_code for v in violations])

    def test_anyof_keyword_is_rejected_as_unsupported(self):
        violations = pr.schema_meta_violations({"type": "object", "anyOf": [{"type": "string"}]})
        self.assertIn("SCHEMA_META_UNSUPPORTED_KEYWORD", [v.reason_code for v in violations])

    def test_bad_shape_is_caught_recursively_inside_properties(self):
        violations = pr.schema_meta_violations({"type": "object", "properties": {"x": {"type": 1}}})
        self.assertIn("SCHEMA_META_BAD_TYPE_KEYWORD", [v.reason_code for v in violations])

    def test_bad_shape_is_caught_recursively_inside_definitions(self):
        violations = pr.schema_meta_violations({"type": "object", "definitions": {"x": {"required": 5}}})
        self.assertIn("SCHEMA_META_BAD_REQUIRED", [v.reason_code for v in violations])


class TestSchemaViolationsNeverRaises(unittest.TestCase):
    """schema_violations must never let ANY schema document crash into a bare traceback -- either
    schema_meta_violations catches the shape problem first, or the generic-engine call itself is
    wrapped."""

    MALFORMED_SCHEMAS = [
        {},
        {"type": 1},
        {"type": "object", "required": 5},
        {"type": "object", "properties": ["not", "dict"]},
        {"type": "object", "definitions": "not-a-dict"},
        {"type": "array", "items": "not-a-schema"},
        {"type": "object", "oneOf": [{"type": "string"}]},
        {"type": "object", "anyOf": [{"type": "string"}]},
    ]

    def test_every_malformed_schema_yields_violations_not_an_exception(self):
        doc = _load_json(REGISTRY_PATH)
        for schema in self.MALFORMED_SCHEMAS:
            with self.subTest(schema=schema):
                violations = pr.schema_violations(doc, schema)
                self.assertGreater(len(violations), 0)

    def test_canonical_schema_still_reports_real_shape_violations_on_bad_instance(self):
        violations = pr.schema_violations({"schema_version": 2, "policies": [{}]}, REAL_SCHEMA)
        self.assertGreater(len(violations), 0)
        self.assertTrue(all(v.reason_code.startswith("SCHEMA:") for v in violations))


class TestCliSchemaTrust(unittest.TestCase):
    """End-to-end via the actual CLI: `--schema {}` must no longer disable validation, and a
    malformed-but-truthy schema must not crash the process."""

    def _run(self, schema_path: Path) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(POLICY_REGISTRY), "verify", "--as-of", "2026-07-25",
             "--schema", str(schema_path)],
            capture_output=True, text=True)

    def test_empty_schema_document_is_rejected_exit2_not_exit0(self):
        with tempfile.TemporaryDirectory() as tmp:
            schema_path = Path(tmp) / "empty.json"
            schema_path.write_text("{}", encoding="utf-8")
            result = self._run(schema_path)
            self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)
            payload = json.loads(result.stdout)
            self.assertTrue(any(v["reason_code"] == "SCHEMA_META_EMPTY_ROOT" for v in payload["violations"]),
                             msg=payload)

    def test_malformed_type_schema_document_is_rejected_exit2_with_stable_json_not_a_traceback(self):
        with tempfile.TemporaryDirectory() as tmp:
            schema_path = Path(tmp) / "bad_type.json"
            schema_path.write_text(json.dumps({"type": 1}), encoding="utf-8")
            result = self._run(schema_path)
            self.assertEqual(result.returncode, 2, msg=result.stdout + result.stderr)
            self.assertNotIn("Traceback", result.stderr)
            payload = json.loads(result.stdout)  # must parse as clean JSON -- no traceback noise
            self.assertEqual(payload["exit_code"], 2)

    def test_canonical_schema_via_cli_still_passes(self):
        result = self._run(SCHEMA_PATH)
        self.assertIn(result.returncode, (0, 1), msg=result.stdout + result.stderr)  # never 2


if __name__ == "__main__":
    unittest.main()
