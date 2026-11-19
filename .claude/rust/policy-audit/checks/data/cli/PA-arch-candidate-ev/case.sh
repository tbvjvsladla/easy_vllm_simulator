# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/arch_variant_ledger.json '.source_build_variants["deepseek-v4-flash-sm12x-port-0.27.1"].evidence = {"plan":"docs/plan/bad.md","stock_infeasible_testlogs":[]} | .source_build_variants["deepseek-v4-flash-sm12x-port-0.27.1"].regression_evidence = {}' || return 1
fx_commit || return 1
}
