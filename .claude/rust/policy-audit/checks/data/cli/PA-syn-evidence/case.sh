# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/reg.json" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_synth || return 1
cat >"$ROOT/r/reg.json" <<'JSON_EOF' || return 1
{
  "schema_version": 2,
  "policies": [
    {
      "policy_id": "TEST_EV",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "TEST_EV.C1",
          "statement": "c"
        },
        {
          "clause_id": "TEST_EV.C2",
          "statement": "c"
        },
        {
          "clause_id": "TEST_EV.C3",
          "statement": "c"
        },
        {
          "clause_id": "TEST_EV.C4",
          "statement": "c"
        }
      ],
      "evidence": [
        {
          "path": "/etc/hostname",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "x"
          ]
        },
        {
          "path": "../outside.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "x"
          ]
        },
        {
          "path": "ev/link.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "evlink/a.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "ev/missing.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "x"
          ]
        },
        {
          "path": "ev/untracked.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "x"
          ]
        },
        {
          "path": "ev/./a.py",
          "supports": [
            "TEST_EV.C1"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "ev/a.py",
          "supports": [
            "TEST_EV.C9"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "ev/c.md",
          "supports": [
            "TEST_EV.C3"
          ],
          "assertion_ids": [
            "Heading One"
          ]
        },
        {
          "path": "ev/c.md",
          "supports": [
            "TEST_EV.C4"
          ],
          "assertion_ids": [
            "Heading One"
          ]
        },
        {
          "path": "ev/a.py",
          "supports": [
            "TEST_EV.C4"
          ],
          "assertion_ids": [
            "alpha"
          ]
        },
        {
          "path": "~/x.py",
          "supports": [
            "TEST_EV.C1"
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
    },
    {
      "policy_id": "TEST_NOEV",
      "owner": "x",
      "statement": "s",
      "scope": [
        "single-node"
      ],
      "origin_failure": "plan_x",
      "clauses": [
        {
          "clause_id": "TEST_NOEV.C1",
          "statement": "c"
        }
      ],
      "evidence": [],
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
ln -s a.py "$ROOT/r/ev/link.py" || return 1
ln -s ev "$ROOT/r/evlink" || return 1
fx_commit || return 1
printf 'X = 1\n' >"$ROOT/r/ev/untracked.py" || return 1
}
