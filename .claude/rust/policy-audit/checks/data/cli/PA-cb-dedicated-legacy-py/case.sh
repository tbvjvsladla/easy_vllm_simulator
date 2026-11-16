# shellcheck shell=bash
# claim bindings: the retired claim_predicates.py no longer counts as the dedicated predicate
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-01-15 --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
mkdir -p "$ROOT/r/.claude/policies/predicates" || return 1
printf '%s\n' 'def predicate_CANON_C1(): pass' >"$ROOT/r/.claude/policies/predicates/claim_predicates.py" || return 1
cat >"$ROOT/r/reg.json" <<'JSON_EOF' || return 1
{
  "schema_version": 2,
  "policies": [
    {
      "policy_id": "CANON",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "CANON.C1",
          "statement": "c"
        }
      ],
      "evidence": [
        {
          "path": "ev/a.py",
          "supports": [
            "CANON.C1"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "ev/b.sh",
          "supports": [
            "CANON.C1"
          ],
          "assertion_ids": [
            "KEY_OK"
          ]
        },
        {
          "path": ".claude/policies/predicates/claim_predicates.py",
          "supports": [
            "CANON.C1"
          ],
          "assertion_ids": [
            "predicate_CANON_C1"
          ]
        }
      ],
      "added_at": "2026-01-01",
      "last_reproduced": null,
      "remove_when": "Removable only once this self-test itself is deleted from the module.",
      "status": "candidate",
      "review": {
        "state": "current",
        "last_reviewed_at": "2026-01-15",
        "next_review_due": "2026-04-15",
        "reviewer": "r"
      }
    }
  ]
}
JSON_EOF
cat >"$ROOT/r/.claude/policies/claim_bindings.json" <<'JSON_EOF' || return 1
{"schema_version": 1, "bindings": {"CANON.C1": [{"path": "ev/a.py", "assertion_ids": ["alpha"]}, {"path": "ev/b.sh", "assertion_ids": ["KEY_OK"]}, {"path": ".claude/policies/predicates/claim_predicates.py", "assertion_ids": ["predicate_CANON_C1"]}]}}
JSON_EOF
fx_commit || return 1
}
