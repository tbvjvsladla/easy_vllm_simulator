# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/arch_variant_ledger.json '(.source_build_variants|to_entries|map(select(.value|type=="object"))|.[0].key) as $k | .source_build_variants[$k].status = "SUPERSEDED in prose"' || return 1
fx_commit || return 1
}
