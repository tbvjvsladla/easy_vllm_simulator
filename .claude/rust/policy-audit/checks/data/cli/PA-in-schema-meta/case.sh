# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/reg.json" --schema "$ROOT/s.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
cat >"$ROOT/r/reg.json" <<'JSON_EOF' || return 1
{
  "schema_version": 2,
  "policies": [
    {
      "policy_id": "TEST_OK",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "TEST_OK.C1",
          "statement": "c"
        }
      ],
      "evidence": [
        {
          "path": "ev/a.py",
          "supports": [
            "TEST_OK.C1"
          ],
          "assertion_ids": [
            "alpha"
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
printf '%s' '{"type":1,"pattern":"x","required":5,"properties":{"a":{"type":["strin"]},"b":3},"definitions":[],"items":{"$ref":"http://x"},"additionalProperties":"no","allOf":{},"if":{"enum":3},"minLength":true}' >"$ROOT/s.json" || return 1
}
