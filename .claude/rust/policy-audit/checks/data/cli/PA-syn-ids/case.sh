# shellcheck shell=bash
# assertion-id extraction per file kind
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-01-15 --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
cat >"$ROOT/r/reg.json" <<'JSON_EOF' || return 1
{
  "schema_version": 2,
  "policies": [
    {
      "policy_id": "TEST_IDS",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "TEST_IDS.C1",
          "statement": "c"
        }
      ],
      "evidence": [
        {
          "path": "ev/a.py",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "alpha",
            "beta",
            "a",
            "helper",
            "Plain.test_two",
            "hidden",
            "K",
            "K.helper"
          ]
        },
        {
          "path": "ev/b.sh",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "KEY_OK",
            "do_thing",
            "topology",
            "RETIRED_VAR"
          ]
        },
        {
          "path": "ev/c.md",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "Heading One",
            "Second heading",
            "prose"
          ]
        },
        {
          "path": "ev/d.yaml",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "top_key",
            "other.key",
            "hidden_key",
            "block"
          ]
        },
        {
          "path": "ev/e.json",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "alpha_key",
            "beta_key",
            "inner"
          ]
        },
        {
          "path": "ev/f.ts",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "tsFn",
            "tsConst",
            "hiddenTs"
          ]
        },
        {
          "path": "ev/g.rs",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "rs_fn",
            "RS_CONST",
            "indented"
          ]
        },
        {
          "path": "ev/h.txt",
          "supports": [
            "TEST_IDS.C1"
          ],
          "assertion_ids": [
            "x"
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
fx_commit || return 1
}
