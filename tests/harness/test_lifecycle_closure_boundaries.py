"""tests/harness/test_lifecycle_closure_boundaries.py -- TDD for two exact cycle3 (subagent-
summary-1 finding 5) lifecycle-closure bugs in scripts/policy_registry.py:

  1. Live (active/candidate) policies carrying an explicit `retirement_reason: null` /
     `superseded_by: null` / `removal_evidence: null` key passed with zero violations, because the
     OLD closure check used truthiness (`if reason: ...`) instead of key PRESENCE. A live policy
     must not carry these keys AT ALL, whatever their value.
  2. review.state was not an EXACT deterministic function of as_of vs next_review_due: at
     as_of == next_review_due, review.state="overdue" (with a substantive reason) ALSO passed
     alongside the correct "due" -- the OLD check only rejected "current" in that window, not
     every state except the one correct one.

Every fixture here is an in-memory deep-copy of valid_baseline.yaml with the SMALLEST field-level
mutation that exercises the rule under test (plan_26072506 Phase4 cycle4 remediation -- see
docs.md/test_policy_registry.py module docstring for why this is preferred over a new static
fixture file per case).

Runner: stdlib `unittest`.
"""
from __future__ import annotations

import copy
import datetime
import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / "scripts"
FIXTURES_DIR = REPO_ROOT / "tests" / "harness" / "fixtures" / "policy_registry"

sys.path.insert(0, str(SCRIPTS_DIR))
import policy_registry as pr  # noqa: E402


def _load_baseline() -> dict:
    with open(FIXTURES_DIR / "valid_baseline.yaml", "r", encoding="utf-8") as f:
        return json.load(f)


def _auto_manifest(doc: dict) -> dict:
    manifest = {}
    for p in doc.get("policies", []):
        for e in p.get("evidence") or []:
            full = REPO_ROOT / e["path"]
            if full.is_file():
                manifest[e["path"]] = pr._sha256_file(full)
    return manifest


AS_OF = datetime.date(2026, 7, 25)


class TestKeyPresenceClosureOnLivePolicies(unittest.TestCase):
    """Exact cycle3 reproduction: `retirement_reason: null` (key PRESENT, value null) on a live
    (candidate/active) policy must be rejected -- the field's mere presence is the violation, not
    its truthiness."""

    def test_explicit_null_retirement_reason_on_a_candidate_policy_is_rejected(self):
        doc = _load_baseline()
        doc["policies"][0]["retirement_reason"] = None
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        codes = [v.reason_code for v in violations]
        self.assertIn("RETIREMENT_METADATA_ON_LIVE_POLICY", codes)
        hit = next(v for v in violations if v.reason_code == "RETIREMENT_METADATA_ON_LIVE_POLICY")
        self.assertEqual(hit.path, f"$.policies[{doc['policies'][0]['policy_id']}].retirement_reason")

    def test_explicit_null_superseded_by_on_a_candidate_policy_is_rejected(self):
        doc = _load_baseline()
        doc["policies"][0]["superseded_by"] = None
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        codes = [v.reason_code for v in violations]
        self.assertIn("RETIREMENT_METADATA_ON_LIVE_POLICY", codes)

    def test_explicit_null_removal_evidence_on_a_candidate_policy_is_rejected(self):
        doc = _load_baseline()
        doc["policies"][0]["removal_evidence"] = None
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        codes = [v.reason_code for v in violations]
        self.assertIn("RETIREMENT_METADATA_ON_LIVE_POLICY", codes)

    def test_baseline_with_no_such_keys_at_all_is_unaffected(self):
        doc = _load_baseline()
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        self.assertEqual([v for v in violations if v.reason_code == "RETIREMENT_METADATA_ON_LIVE_POLICY"], [])

    def test_real_registry_has_no_live_policy_carrying_these_keys_by_presence(self):
        with open(REPO_ROOT / ".claude" / "policies" / "registry.yaml", encoding="utf-8") as f:
            registry = json.load(f)
        for p in registry["policies"]:
            if p["status"] in ("active", "candidate"):
                with self.subTest(policy_id=p["policy_id"]):
                    self.assertNotIn("retirement_reason", p)
                    self.assertNotIn("superseded_by", p)
                    self.assertNotIn("removal_evidence", p)


class TestAsOfReviewStateExactBoundary(unittest.TestCase):
    """Exact cycle3 reproduction: at as_of == next_review_due, review.state="overdue" (not "due")
    must now be rejected -- review.state is an EXACT deterministic function of as_of vs
    next_review_due (before due=current; due <= as_of <= due+30d => due; after due+30d =>
    overdue), not "current is wrong, anything else is fine"."""

    def _doc_with_review(self, state: str, last_reviewed_at: str, next_review_due: str,
                          reason: str | None = "Boundary-precision regression test reason with enough words."):
        doc = _load_baseline()
        review = {"state": state, "last_reviewed_at": last_reviewed_at, "next_review_due": next_review_due,
                   "reviewer": "test"}
        if reason is not None:
            review["reason"] = reason
        doc["policies"][0]["review"] = review
        return doc

    def test_overdue_at_exactly_next_review_due_is_rejected_not_accepted(self):
        # last_reviewed_at 90 days before as_of => next_review_due == as_of exactly.
        last_reviewed = (AS_OF - datetime.timedelta(days=pr.REVIEW_PERIOD_DAYS)).isoformat()
        doc = self._doc_with_review("overdue", last_reviewed, AS_OF.isoformat())
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        hits = [v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"]
        self.assertEqual(len(hits), 1, msg=[v.to_dict() for v in violations])
        self.assertIn("due", hits[0].message)

    def test_due_at_exactly_next_review_due_is_accepted(self):
        last_reviewed = (AS_OF - datetime.timedelta(days=pr.REVIEW_PERIOD_DAYS)).isoformat()
        doc = self._doc_with_review("due", last_reviewed, AS_OF.isoformat())
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        self.assertEqual([v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"], [])

    def test_due_at_exactly_the_30_day_grace_boundary_is_still_accepted_inclusive(self):
        due = (AS_OF - datetime.timedelta(days=pr.OVERDUE_GRACE_DAYS)).isoformat()
        last_reviewed = (AS_OF - datetime.timedelta(days=pr.REVIEW_PERIOD_DAYS + pr.OVERDUE_GRACE_DAYS)).isoformat()
        doc = self._doc_with_review("due", last_reviewed, due)
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        self.assertEqual([v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"], [])

    def test_overdue_one_day_past_the_30_day_grace_boundary_is_required(self):
        due = (AS_OF - datetime.timedelta(days=pr.OVERDUE_GRACE_DAYS + 1)).isoformat()
        last_reviewed = (AS_OF - datetime.timedelta(days=pr.REVIEW_PERIOD_DAYS + pr.OVERDUE_GRACE_DAYS + 1)) \
            .isoformat()
        doc = self._doc_with_review("due", last_reviewed, due)  # claims "due" but should be "overdue"
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        hits = [v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"]
        self.assertEqual(len(hits), 1)
        self.assertIn("overdue", hits[0].message)

    def test_current_before_due_is_accepted(self):
        due = (AS_OF + datetime.timedelta(days=1)).isoformat()
        last_reviewed = AS_OF.isoformat()
        doc = self._doc_with_review("current", last_reviewed, due, reason=None)
        violations = pr.evaluate_lifecycle(doc, AS_OF, REPO_ROOT, _auto_manifest(doc))
        self.assertEqual([v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"], [])

    def test_real_registry_has_zero_as_of_state_mismatches_at_its_own_review_date(self):
        with open(REPO_ROOT / ".claude" / "policies" / "registry.yaml", encoding="utf-8") as f:
            registry = json.load(f)
        violations = pr.evaluate_lifecycle(registry, AS_OF, REPO_ROOT)
        self.assertEqual([v for v in violations if v.reason_code == "AS_OF_STATE_MISMATCH"], [])


if __name__ == "__main__":
    unittest.main()
