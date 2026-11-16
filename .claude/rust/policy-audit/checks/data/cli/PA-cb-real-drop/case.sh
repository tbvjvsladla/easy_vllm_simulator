# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of 2026-11-16 --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/claim_bindings.json 'del(.bindings[(.bindings|keys)[0]]) | .bindings[(.bindings|keys)[1]][0].assertion_ids += ["extra_id"]' || return 1
fx_commit || return 1
}
