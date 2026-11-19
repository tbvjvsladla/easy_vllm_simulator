# shellcheck shell=bash
PROG_KIND=policy_registry
ARGS=(verify --registry "$ROOT/r/.claude/policies/registry.yaml" --schema "$ROOT/r/.claude/schemas/policy-registry.schema.json" --repo-root "$ROOT/r")
setup() {
. "$SDIR/fixture_lib.sh"
fx_real || return 1
fx_jq .claude/policies/arch_variant_ledger.json '.source_build_variants["deepseek-v4-flash-sm12x-port-0.27.1"].build_patch_selectors |= with_entries(.value="0") | .source_build_variants["deepseek-v4-flash-sm12x-port-0.27.1"].build_patch_selectors.lower_bad = "1"' || return 1
fx_commit || return 1
}
