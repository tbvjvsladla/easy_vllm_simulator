# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry reg.json --repo-root fx --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json")
setup() {
. "$SDIR/fixture_lib.sh"
mkdir -p "$ROOT/r" "$ROOT/fx/.claude/policies" || return 1
fx_from_rev .claude/schemas/policy-registry.schema.json || return 1
printf '%s\n' '{"schema_version": 2, "policies": [{"policy_id": "TEST_A", "owner": "x", "statement": "s", "scope": ["single-node"], "origin_failure": "p", "clauses": [{"clause_id": "TEST_A.C1", "statement": "c"}], "evidence": [{"path": "ev.sh", "supports": ["TEST_A.C1"], "assertion_ids": ["f"]}], "added_at": "2025-01-01", "last_reproduced": "never", "remove_when": "tbd", "status": "active", "review": {"state": "current", "last_reviewed_at": "2025-01-01", "next_review_due": "2025-04-01", "reviewer": "r"}}]}' >"$ROOT/reg.json" || return 1
printf '%s\n' 'f() { :; }' >"$ROOT/fx/ev.sh" || return 1
printf '%s\n' '{"schema_version": 1, "bindings": {}}' >"$ROOT/fx/.claude/policies/claim_bindings.json" || return 1
}
