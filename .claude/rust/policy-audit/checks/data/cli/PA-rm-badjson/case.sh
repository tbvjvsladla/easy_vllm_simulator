# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(check-removal --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r" --policy-id TEST_RET)
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
cat >"$ROOT/r/reg.json" <<'JSON_EOF' || return 1
{
  "schema_version": 2,
  "policies": [
    {
      "policy_id": "TEST_RET",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "TEST_RET.C1",
          "statement": "c"
        }
      ],
      "evidence": [
        {
          "path": "ev/a.py",
          "supports": [
            "TEST_RET.C1"
          ],
          "assertion_ids": [
            "alpha"
          ]
        }
      ],
      "added_at": "2026-01-01",
      "last_reproduced": null,
      "remove_when": "Removable only once this self-test itself is deleted from the module.",
      "status": "retired",
      "review": {
        "state": "current",
        "last_reviewed_at": "2026-01-15",
        "next_review_due": "2026-04-15",
        "reviewer": "r"
      },
      "retirement_reason": "Retired because the guarded mechanism was deleted from every caller in the tree.",
      "removal_evidence": {
        "status": "contract_e2e_green",
        "path": "rm/art.json"
      }
    }
  ]
}
JSON_EOF
mkdir -p "$ROOT/r/rm" || return 1
printf '[1,' >"$ROOT/r/rm/art.json" || return 1
fx_commit || return 1
}
